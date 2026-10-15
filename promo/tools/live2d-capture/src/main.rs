//! Renders a Live2D model offscreen to a transparent PNG sequence, driven by the same event
//! timeline as the promo VTS scene (speaking 10..84, smile at 40, shake 58..70; 30 fps).
//!
//! cargo run --release -- <model3.json> <out-dir> [frames=135] [size=800]

use live2d_runtime::model::MotionSelector;
use live2d_runtime::motion::MotionPlaybackOptions;
use live2d_runtime::parameter::IndexedParameterWrite;
use live2d_wgpu::{RegistrationRequest, RenderTarget, RenderView, Renderer, RendererOptions, SubmissionBatch};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let model = args.get(1).ok_or("usage: live2d-capture <model3.json> <out-dir> [frames] [size]")?;
    let out = std::path::PathBuf::from(args.get(2).ok_or("missing out dir")?);
    let frames: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(135);
    let size: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(800);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let mut instance = live2d_runtime::Instance::load_file(model).map_err(|e| e.to_string())?;
    // Keep the output deterministic: no blink/breath randomness.
    let disabled = live2d_runtime::behavior::BehaviorLayerOptions { enabled: false, ..Default::default() };
    instance.set_eye_blink_options(disabled);
    instance.set_breath_options(disabled);
    if instance.motion_group("Idle").is_some() {
        instance
            .set_base_motion(
                MotionSelector::index("Idle", 0),
                MotionPlaybackOptions { loop_playback: true, fade_in_seconds: 0.5, fade_out_seconds: 0.5, speed: 1.0, ..MotionPlaybackOptions::default() },
            )
            .map_err(|e| e.to_string())?;
    }
    let smile_ids = ["ParamEyeLSmile", "ParamEyeRSmile", "ParamMouthForm", "ParamCheek"];
    let smile: Vec<_> = smile_ids.iter().filter_map(|id| instance.parameter_index(id)).collect();
    let expression = args.get(5).cloned();
    let mouth = instance.parameter_index("ParamMouthOpenY").ok_or("no ParamMouthOpenY")?;
    let angle_x = instance.parameter_index("ParamAngleX").ok_or("no ParamAngleX")?;
    let angle_z = instance.parameter_index("ParamAngleZ").ok_or("no ParamAngleZ")?;

    // ---- GPU
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12 | wgpu::Backends::VULKAN;
    let wg = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(wg.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("adapter: {e}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("live2d-capture"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("device: {e}"))?;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());

    let mut renderer = Renderer::new(&device, RendererOptions::sdr(wgpu::TextureFormat::Rgba8UnormSrgb)).map_err(|e| e.to_string())?;
    let handle = renderer
        .register(&device, &queue, RegistrationRequest::new(instance.static_model()))
        .map_err(|e| format!("register: {e}"))?
        .handle;
    let placement = live2d_wgpu::fit_model_to_viewport(
        live2d_wgpu::RectU32 { min: [0, 0], max: [size, size] },
        instance.static_model().canvas.model_bounds(),
        live2d_core::Anchor::Center,
        8.0,
    );
    let view = RenderView::from_static_model_placement(instance.static_model(), placement, size, size);

    let padded = (size * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    // Warm up so the idle motion has faded in before the first captured frame.
    for _ in 0..30 {
        instance.update_with_pinned_parameters(DT, &[]).map_err(|e| e.to_string())?;
    }
    for f in 0..frames {
        if f == 40 {
            if let Some(name) = expression.as_deref() {
                instance.apply_expression_by_name(name).map_err(|e| e.to_string())?;
            }
        }
        let speaking = (10..84).contains(&f);
        let mouth_v = if speaking { (0.15 + 0.85 * ((f as f32) * 1.1).sin().abs()).min(1.0) } else { 0.0 };
        let (mut ax, mut az) = (0.0f32, 0.0f32);
        if (58..70).contains(&f) {
            let t = (f - 58) as f32;
            let k = 1.0 - t / 12.0;
            ax = (t * 2.4).sin() * 22.0 * k;
            az = (t * 2.4 + 1.0).sin() * 10.0 * k;
        }
        let mut writes = vec![
            IndexedParameterWrite { target: mouth, value: mouth_v },
            IndexedParameterWrite { target: angle_x, value: ax },
            IndexedParameterWrite { target: angle_z, value: az },
        ];
        // Smile from frame 40 (model has no .exp3 listed in model3.json: pin the smile parameters).
        if expression.is_none() && f >= 40 {
            let k = ((f - 40) as f32 / 6.0).min(1.0);
            for t in &smile {
                writes.push(IndexedParameterWrite { target: *t, value: k });
            }
        }
        let frame = instance.update_with_pinned_parameters(DT, writes.as_slice()).map_err(|e| e.to_string())?.frame;
        renderer.update_model(&queue, handle, &frame).map_err(|e| e.to_string())?;
        let mut encoder = device.create_command_encoder(&Default::default());
        let prepared = renderer.prepare_model(&device, &queue, &mut encoder, handle, view).map_err(|e| e.to_string())?;
        let encoded = renderer
            .encode_model(
                &device,
                &queue,
                &mut encoder,
                RenderTarget::clear(&target, &target_view, wgpu::Color::TRANSPARENT),
                &prepared,
            )
            .map_err(|e| e.to_string())?;
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        renderer
            .complete_submission(SubmissionBatch::from_token(encoded.submission), std::time::Duration::ZERO)
            .map_err(|e| e.to_string())?;

        // read back
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded * size),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(size) },
            },
            wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        );
        queue.submit([enc.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let data = slice.get_mapped_range().map_err(|e| format!("{e:?}"))?;
        let mut px = Vec::with_capacity((size * size * 4) as usize);
        for row in data.chunks(padded as usize).take(size as usize) {
            px.extend_from_slice(&row[..(size * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        // Renderer output is premultiplied; PNG wants straight alpha.
        for p in px.chunks_exact_mut(4) {
            let a = p[3] as f32 / 255.0;
            if a > 0.0 {
                for c in &mut p[..3] {
                    *c = ((*c as f32) / a).min(255.0) as u8;
                }
            }
        }
        image::save_buffer(out.join(format!("{f:04}.png")), &px, size, size, image::ColorType::Rgba8).map_err(|e| e.to_string())?;
    }
    println!("wrote {frames} frames to {}", out.display());
    Ok(())
}
