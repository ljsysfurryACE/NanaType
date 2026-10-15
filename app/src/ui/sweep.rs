//! A cached coverage mask and one in-pass draw, under the dialogue's text.
//! The host owns the device, submission and output targets. Only 48 uniform
//! bytes change on an animated frame; the retained node and mask stay put.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Weak};

use nana_ui::runtime::view::{AnyView, IntoView, widget};
use nana_ui::runtime::{
    ComponentView, CustomRenderNode, GpuView, MutationQueue, NodeKind, StableNodeId, UiWorld,
};
use nana_ui::{
    DeviceGeneration, GpuTextureFormat, SceneGpuNode, SceneGpuPassContext, SceneGpuPrepareContext,
    SceneGpuRenderContext, SceneGpuRenderer, SceneGpuRendererRegistry, ScenePass,
    ScenePresentationProfile, wgpu,
};
use nana_ui_core::{LengthSpec, PointerEventsSpec, PositionSpec};
use nanatype_theme::{ShapeCommand, SweepSpec, contour_path};

use super::surface::Surface;

pub fn animating(ui: super::Ui) -> bool {
    !ui.reduced_motion.get_untracked()
        && !ui.display.with_untracked(|d| d.empty)
        && ui.skin.with_untracked(|s| {
            s.dialogue.sweep.enabled
                && s.dialogue.sweep.intensity > 0.0
                && s.dialogue.sweep.color.a > 0
        })
}

const RENDERER: &str = "nanatype.dialogue-sweep";
static NEXT: AtomicU64 = AtomicU64::new(1);
static MATERIALS: LazyLock<Mutex<HashMap<u64, Weak<Material>>>> = LazyLock::new(Default::default);

#[derive(Debug, PartialEq)]
struct Material {
    id: u64,
    surface: Surface,
    spec: SweepSpec,
    animated: bool,
}
impl Drop for Material {
    fn drop(&mut self) {
        MATERIALS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

#[derive(Clone, PartialEq)]
struct SweepView {
    base: GpuView,
    material: Arc<Material>,
}
impl ComponentView for SweepView {
    fn share_layouts(&mut self, share: &mut dyn FnMut(&mut Arc<nana_ui_core::LayoutStyle>)) {
        self.base.share_layouts(share);
    }
    fn node_kind(&self) -> NodeKind {
        self.base.node_kind()
    }
    fn project(&self, id: StableNodeId, world: &UiWorld, mutations: &mut MutationQueue) {
        self.base.project(id, world, mutations);
        mutations.set_custom_render(
            id,
            Some(CustomRenderNode::new(
                RENDERER,
                self.material.id.to_string(),
                1,
            )),
        );
    }
}

pub fn view(surface: Surface, spec: SweepSpec, animated: bool) -> AnyView {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let material = Arc::new(Material {
        id,
        surface,
        spec,
        animated,
    });
    MATERIALS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, Arc::downgrade(&material));
    let mut base = GpuView::new(id);
    let l = Arc::make_mut(&mut base.style.layout);
    l.position = PositionSpec::Absolute;
    l.offset_left = Some(LengthSpec::Px(0.0));
    l.offset_top = Some(LengthSpec::Px(0.0));
    l.width = Some(LengthSpec::Percent(100.0));
    l.height = Some(LengthSpec::Percent(100.0));
    l.pointer_events = Some(PointerEventsSpec::None);
    widget(SweepView { base, material }).into_any()
}

/// The same registry is used by native windows, OBS output and snapshot tools.
pub fn renderers() -> SceneGpuRendererRegistry {
    static REGISTRY: LazyLock<SceneGpuRendererRegistry> = LazyLock::new(|| {
        let mut registry = nana_ui::default_scene_gpu_renderers();
        registry.insert(RENDERER, Arc::new(SweepRenderer::default()));
        registry
    });
    REGISTRY.clone()
}

type PipelineKey = (DeviceGeneration, GpuTextureFormat, ScenePresentationProfile);
type MaskKey = (PipelineKey, u64, [u32; 2], u32);
#[derive(Debug)]
struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}
#[derive(Debug)]
struct Prepared {
    material: Weak<Material>,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    mask_bytes: usize,
}
#[derive(Debug, Default)]
struct State {
    pipelines: HashMap<PipelineKey, Pipeline>,
    masks: HashMap<MaskKey, Prepared>,
    scales: HashMap<(DeviceGeneration, u64), f32>,
}
#[derive(Debug, Default)]
struct SweepRenderer {
    state: Mutex<State>,
}

fn bytes(values: &[f32; 12]) -> [u8; 48] {
    let mut bytes = [0; 48];
    for (dest, value) in bytes.chunks_exact_mut(4).zip(values) {
        dest.copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}
fn linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

impl State {
    fn prepare_mask(
        &mut self,
        material: &Arc<Material>,
        context: &SceneGpuPassContext<'_>,
        scale: f32,
    ) -> &Prepared {
        let pipeline_key = (
            context.gpu.generation(),
            context.target_format,
            context.presentation,
        );
        let extent = [context.bounds.width.max(1), context.bounds.height.max(1)];
        // Coverage depends on size and DPI, not the destination viewport.
        let key = (pipeline_key, material.id, extent, scale.to_bits());
        if self.masks.contains_key(&key) {
            return &self.masks[&key];
        }
        self.masks
            .retain(|_, value| value.material.strong_count() > 0);
        // Bound stale resize/scale variants without growing during a long session.
        if self.masks.len() >= 32
            || self.masks.values().map(|m| m.mask_bytes).sum::<usize>()
                + extent[0] as usize * extent[1] as usize * 4
                > 64 * 1024 * 1024
        {
            self.masks.clear();
        }
        self.pipelines
            .retain(|(generation, _, _), _| *generation == context.gpu.generation());
        let device = context.gpu.wgpu().device();
        let pipeline = self.pipelines.entry(pipeline_key).or_insert_with(|| {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("dialogue sweep"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("dialogue sweep"),
                source: wgpu::ShaderSource::Wgsl(include_str!("sweep.wgsl").into()),
            });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("dialogue sweep"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("dialogue sweep"),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: context.target_format.wgpu(),
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
            Pipeline { pipeline, layout }
        });
        let mask = coverage(&material.surface, extent, scale);
        let mask_bytes = mask.len();
        if let Some(work) = context.gpu_work {
            work.record_upload(mask_bytes);
            work.record_realloc();
        }
        let texture = context
            .gpu
            .create_texture(&nana_ui::GpuTextureDescriptor {
                label: Some("dialogue sweep coverage"),
                width: extent[0],
                height: extent[1],
                format: GpuTextureFormat::RGBA8_UNORM,
                usage: nana_ui::GpuTextureUsages::SAMPLED | nana_ui::GpuTextureUsages::COPY_DST,
            })
            .expect("validated sweep extent");
        context
            .gpu
            .write_texture(
                &texture,
                nana_ui::GpuTextureRegion {
                    x: 0,
                    y: 0,
                    width: extent[0],
                    height: extent[1],
                },
                &mask,
                extent[0] * 4,
            )
            .expect("coverage upload");
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dialogue sweep parameters"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("dialogue sweep mask"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dialogue sweep"),
            layout: &pipeline.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture.wgpu_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let pipeline = pipeline.pipeline.clone();
        self.masks.entry(key).or_insert_with(|| Prepared {
            material: Arc::downgrade(material),
            uniform,
            group,
            pipeline,
            mask_bytes,
        })
    }
}

fn material(node: &SceneGpuNode) -> Option<Arc<Material>> {
    let id = node.custom.resource.parse().ok()?;
    MATERIALS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&id)?
        .upgrade()
}

impl SceneGpuRenderer for SweepRenderer {
    fn preparation_version(&self, node: &CustomRenderNode) -> Option<u64> {
        Some(node.revision)
    }
    fn prepare(&self, node: &SceneGpuNode, context: SceneGpuPrepareContext<'_>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.scales.insert(
            (context.gpu.generation(), node.id.node.get()),
            context.scale_factor,
        );
    }
    fn render(&self, node: &SceneGpuNode, mut context: SceneGpuRenderContext<'_>) {
        let pass_context = SceneGpuPassContext {
            gpu: context.gpu,
            target_format: context.target_format,
            presentation: context.presentation,
            bounds: context.bounds,
            clip: context.clip,
            dest_size: context.dest_size,
            gpu_work: context.gpu_work,
        };
        context.with_pass("dialogue sweep", |pass| {
            self.draw_in_pass(node, pass, pass_context);
        });
    }
    fn draw_in_pass(
        &self,
        node: &SceneGpuNode,
        pass: &mut ScenePass<'_, '_>,
        context: SceneGpuPassContext<'_>,
    ) -> bool {
        let Some(material) = material(node) else {
            return true;
        };
        if context.bounds.width == 0 || context.bounds.height == 0 {
            return true;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let scale = state
            .scales
            .get(&(context.gpu.generation(), node.id.node.get()))
            .copied()
            .unwrap_or(1.0);
        let prepared = state.prepare_mask(&material, &context, scale);
        let c = material.spec.color;
        let phase = if material.animated {
            (crate::clock::now_ms() as f64 / 1000.0 / f64::from(material.spec.period_s)).fract()
                as f32
        } else {
            0.5
        };
        let values = [
            context.dest_size[0] as f32,
            context.dest_size[1] as f32,
            context.bounds.x as f32,
            context.bounds.y as f32,
            context.bounds.width as f32,
            context.bounds.height as f32,
            phase,
            node.opacity * material.spec.intensity,
            linear(c.r),
            linear(c.g),
            linear(c.b),
            f32::from(c.a) / 255.0,
        ];
        let upload = bytes(&values);
        let interop = context.gpu.wgpu();
        let _submission = interop.lock_submission();
        interop.queue().write_buffer(&prepared.uniform, 0, &upload);
        if let Some(work) = context.gpu_work {
            work.record_upload(upload.len());
            work.record_draw_call();
        }
        let raw = pass.wgpu();
        raw.set_pipeline(&prepared.pipeline);
        raw.set_bind_group(0, &prepared.group, &[]);
        raw.draw(0..6, 0..1);
        true
    }
}

/// Static alpha coverage, built once per physical size and surface revision.
fn coverage(surface: &Surface, size: [u32; 2], scale: f32) -> Vec<u8> {
    let [w, h] = size;
    let logical = [w as f32 / scale, h as f32 / scale];
    if let Some(mask) = frame_coverage(surface, size, scale) {
        return mask;
    }
    let mut builder = tiny_skia::PathBuilder::new();
    let mut even_odd = false;
    if let Some(contour) = surface.contour {
        let border = surface
            .border
            .as_ref()
            .map_or(0.0, |b| b.widths.into_iter().fold(0.0, f32::max));
        let shape = contour_path(
            contour,
            logical,
            surface.contour_radius(),
            border * 0.5,
            surface.tail.as_ref(),
        );
        even_odd = shape.even_odd;
        for command in shape.commands {
            match command {
                ShapeCommand::Move([x, y]) => builder.move_to(x, y),
                ShapeCommand::Line([x, y]) => builder.line_to(x, y),
                ShapeCommand::Quad([a, b], [x, y]) => builder.quad_to(a, b, x, y),
                ShapeCommand::Cubic([a, b], [c, d], [x, y]) => builder.cubic_to(a, b, c, d, x, y),
                ShapeCommand::Close => builder.close(),
            }
        }
    } else {
        let [w, h] = logical;
        let [tl, tr, br, bl] = surface.radii.map(|v| v.max(0.0).min(w.min(h) * 0.5));
        let k = 0.5522848;
        builder.move_to(tl, 0.0);
        builder.line_to(w - tr, 0.0);
        builder.cubic_to(w - tr + k * tr, 0.0, w, tr - k * tr, w, tr);
        builder.line_to(w, h - br);
        builder.cubic_to(w, h - br + k * br, w - br + k * br, h, w - br, h);
        builder.line_to(bl, h);
        builder.cubic_to(bl - k * bl, h, 0.0, h - bl + k * bl, 0.0, h - bl);
        builder.line_to(0.0, tl);
        builder.cubic_to(0.0, tl - k * tl, tl - k * tl, 0.0, tl, 0.0);
        builder.close();
    }
    let mut pixmap = tiny_skia::Pixmap::new(w, h).expect("positive sweep size");
    if let Some(path) = builder.finish() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        pixmap.fill_path(
            &path,
            &paint,
            if even_odd {
                tiny_skia::FillRule::EvenOdd
            } else {
                tiny_skia::FillRule::Winding
            },
            tiny_skia::Transform::from_scale(scale, scale),
            None,
        );
    }
    let polygon = surface.polygon.as_ref().and_then(|points| {
        let mut builder = tiny_skia::PathBuilder::new();
        for (i, [x, y]) in points.iter().copied().enumerate() {
            let (x, y) = (x * w as f32 / 100.0, y * h as f32 / 100.0);
            if i == 0 {
                builder.move_to(x, y);
            } else {
                builder.line_to(x, y);
            }
        }
        builder.close();
        let path = builder.finish()?;
        let mut p = tiny_skia::Pixmap::new(w, h)?;
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        p.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            tiny_skia::Transform::identity(),
            None,
        );
        Some(p.take())
    });
    let mut data = pixmap.take();
    for (i, p) in data.chunks_exact_mut(4).enumerate() {
        let mut alpha = surface.fill.map_or(0.0, |c| c[3]);
        if let Some(nana_ui_core::BackgroundImage::Gradient(gradient)) = &surface.gradient {
            let point = [
                ((i as u32 % w) as f32 + 0.5) / scale,
                ((i as u32 / w) as f32 + 0.5) / scale,
            ];
            let (at, stops) = match gradient {
                nana_ui_core::CssGradient::Linear(g) => {
                    let angle = g.angle_deg.to_radians();
                    let dir = [angle.sin(), -angle.cos()];
                    let span = dir[0].abs() * logical[0] + dir[1].abs() * logical[1];
                    (
                        0.5 + ((point[0] - logical[0] * 0.5) * dir[0]
                            + (point[1] - logical[1] * 0.5) * dir[1])
                            / span.max(0.001),
                        &g.stops,
                    )
                }
                nana_ui_core::CssGradient::Radial(g) => {
                    let center = g
                        .resolved_center(logical[0], logical[1])
                        .unwrap_or([0.5, 0.5]);
                    let center = [center[0] * logical[0], center[1] * logical[1]];
                    let (dx, dy) = (
                        center[0].max(logical[0] - center[0]).max(0.1),
                        center[1].max(logical[1] - center[1]).max(0.1),
                    );
                    let (rx, ry) = if g.circle {
                        let r = dx.hypot(dy);
                        (r, r)
                    } else {
                        (dx * 2.0_f32.sqrt(), dy * 2.0_f32.sqrt())
                    };
                    (
                        ((point[0] - center[0]) / rx).hypot((point[1] - center[1]) / ry),
                        &g.stops,
                    )
                }
            };
            let g = gradient_alpha(stops, at);
            alpha = g + alpha * (1.0 - g);
        }
        if let Some(polygon) = &polygon {
            alpha *= f32::from(polygon[i * 4 + 3]) / 255.0;
        }
        p[3] = (f32::from(p[3]) * alpha.clamp(0.0, 1.0)).round() as u8;
    }
    data
}

fn gradient_alpha(stops: &[nana_ui_core::GradientStop], t: f32) -> f32 {
    let Some(first) = stops.first() else {
        return 0.0;
    };
    if t <= first.position {
        return first.color[3];
    }
    for pair in stops.windows(2) {
        if t <= pair[1].position {
            let f = ((t - pair[0].position) / (pair[1].position - pair[0].position).max(0.0001))
                .clamp(0.0, 1.0);
            return pair[0].color[3] * (1.0 - f) + pair[1].color[3] * f;
        }
    }
    stops.last().unwrap().color[3]
}

fn frame_coverage(surface: &Surface, size: [u32; 2], scale: f32) -> Option<Vec<u8>> {
    let (frame, widths) = surface.frame.as_ref()?;
    let nana_ui_core::BackgroundImage::Url { url, .. } = &frame.source else {
        return None;
    };
    let raw = url.strip_prefix("file://")?;
    let raw = if cfg!(windows) && raw.as_bytes().get(2) == Some(&b':') {
        &raw[1..]
    } else {
        raw
    };
    let path = raw
        .replace("%20", " ")
        .replace("%23", "#")
        .replace("%3F", "?")
        .replace("%25", "%");
    let path = std::path::Path::new(&path);
    // These are already resolved media-library URLs. Keep the decoder limits
    // identical to the product image boundary; a missing frame emits no light.
    if std::fs::metadata(path).ok()?.len() > crate::services::media::MAX_FILE_BYTES {
        return Some(vec![0; (size[0] * size[1] * 4) as usize]);
    }
    let dimensions = imagesize::size(path).ok()?;
    if dimensions.width > nanatype_theme::MAX_IMAGE_SIDE
        || dimensions.height > nanatype_theme::MAX_IMAGE_SIDE
    {
        return Some(vec![0; (size[0] * size[1] * 4) as usize]);
    }
    let image = image::open(path).ok()?.to_rgba8();
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let slices = [
        frame.slice[0].to_px(ih),
        frame.slice[1].to_px(iw),
        frame.slice[2].to_px(ih),
        frame.slice[3].to_px(iw),
    ];
    let axis = |value: f32, extent: f32, near: f32, far: f32, src: f32, snear: f32, sfar: f32| {
        let factor = (extent / (near + far).max(0.001)).min(1.0);
        let (near, far) = (near * factor, far * factor);
        if value < near {
            value / near.max(0.001) * snear
        } else if value >= extent - far {
            src - sfar + (value - (extent - far)) / far.max(0.001) * sfar
        } else {
            snear
                + (value - near) / (extent - near - far).max(0.001) * (src - snear - sfar).max(0.0)
        }
    };
    let mut out = vec![0; (size[0] * size[1] * 4) as usize];
    for y in 0..size[1] {
        for x in 0..size[0] {
            let sx = axis(
                (x as f32 + 0.5) / scale,
                size[0] as f32 / scale,
                widths[3],
                widths[1],
                iw,
                slices[3],
                slices[1],
            );
            let sy = axis(
                (y as f32 + 0.5) / scale,
                size[1] as f32 / scale,
                widths[0],
                widths[2],
                ih,
                slices[0],
                slices[2],
            );
            let center =
                sx >= slices[3] && sx < iw - slices[1] && sy >= slices[0] && sy < ih - slices[2];
            if frame.fill || !center {
                out[((y * size[0] + x) * 4 + 3) as usize] = image.get_pixel(
                    (sx.floor() as u32).min(image.width() - 1),
                    (sy.floor() as u32).min(image.height() - 1),
                )[3];
            }
        }
    }
    Some(out)
}
