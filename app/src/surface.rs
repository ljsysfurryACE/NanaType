//! One offscreen BGRA target per lane. The window samples it; Spout publishes
//! the same pixels, so hiding the window does not stop a sender that is on.

use nana_ui::{
    GpuContext, GpuTexture, GpuTextureDescriptor, GpuTextureFormat, GpuTextureRegion,
    GpuTextureUsages, HostTexture, HostTextureAlphaMode, HostTextureRegistry,
};

const SLOT: &str = "dialogue";

pub struct Lane {
    registry: HostTextureRegistry,
    texture: Option<GpuTexture>,
    generation: u64,
    width: u32,
    height: u32,
    name: String,
    error: String,
    #[cfg(windows)]
    sender: Option<nanalive_spout::CpuDx11Sender>,
}

impl Lane {
    pub fn new() -> Self {
        Self {
            registry: HostTextureRegistry::new(),
            texture: None,
            generation: 1,
            width: 0,
            height: 0,
            name: String::new(),
            error: String::new(),
            #[cfg(windows)]
            sender: None,
        }
    }

    pub fn registry(&self) -> HostTextureRegistry {
        self.registry.clone()
    }

    pub fn error(&self) -> &str {
        &self.error
    }

    pub fn upload(&mut self, gpu: &GpuContext, pixels: &[u8], width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.texture.is_none() || self.width != width || self.height != height {
            match gpu.create_texture(&GpuTextureDescriptor {
                label: Some("nanatype-dialogue"),
                width,
                height,
                format: GpuTextureFormat::BGRA8_UNORM,
                usage: GpuTextureUsages::SAMPLED
                    | GpuTextureUsages::COPY_DST
                    | GpuTextureUsages::COPY_SRC,
            }) {
                Ok(texture) => {
                    self.generation = self.generation.wrapping_add(1);
                    self.registry.register(
                        SLOT,
                        HostTexture::new(1, self.generation, &texture),
                        width,
                        height,
                        HostTextureAlphaMode::Premultiplied,
                    );
                    self.texture = Some(texture);
                    self.width = width;
                    self.height = height;
                }
                Err(error) => {
                    self.error = error.to_string();
                    return;
                }
            }
        }
        let Some(texture) = self.texture.as_ref() else {
            return;
        };
        let stride = width.saturating_mul(4);
        if pixels.len() < (stride as usize).saturating_mul(height as usize) {
            return;
        }
        if let Err(error) = gpu.write_texture(
            texture,
            GpuTextureRegion::full(width, height),
            pixels,
            stride,
        ) {
            self.error = error.to_string();
            return;
        }
        self.registry.invalidate(SLOT);
    }

    pub fn publish(&mut self, enabled: bool, name: &str, pixels: &[u8], width: u32, height: u32) {
        if !enabled {
            self.release_sender();
            return;
        }
        #[cfg(windows)]
        {
            if self.name != name || self.sender.is_none() {
                self.name = name.to_owned();
                match nanalive_spout::CpuDx11Sender::new(name) {
                    Ok(sender) => {
                        self.sender = Some(sender);
                        self.error.clear();
                    }
                    Err(error) => {
                        self.sender = None;
                        self.error = format!("Spout 未连接：{error}");
                        return;
                    }
                }
            }
            let Some(sender) = self.sender.as_mut() else {
                return;
            };
            if let Err(error) = nanalive_spout::SpoutSenderBackend::resize_or_recreate(
                sender,
                width,
                height,
                nanalive_spout::SpoutFormat::B8G8R8A8_UNORM,
            ) {
                self.error = format!("Spout 尺寸：{error}");
                return;
            }
            let frame = nanalive_spout::SpoutFrameRef::CpuPixels {
                pixels,
                width,
                height,
                pitch_bytes: None,
            };
            if let Err(error) = nanalive_spout::SpoutSenderBackend::publish(sender, frame) {
                self.error = format!("Spout 发送：{error}");
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (name, pixels, width, height);
            self.error = "Spout 仅在 Windows 上输出".into();
        }
    }

    fn release_sender(&mut self) {
        #[cfg(windows)]
        if let Some(sender) = self.sender.as_mut() {
            nanalive_spout::SpoutSenderBackend::release(sender);
        }
        #[cfg(windows)]
        {
            self.sender = None;
        }
        self.name.clear();
    }
}

impl Default for Lane {
    fn default() -> Self {
        Self::new()
    }
}

pub fn ensure_placeholder(lane: &mut Lane, gpu: &GpuContext) {
    if lane.texture.is_some() {
        return;
    }
    let pixel = [0u8, 0, 0, 0];
    lane.upload(gpu, &pixel, 1, 1);
}
