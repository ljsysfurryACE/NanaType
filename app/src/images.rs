//! Still images via `image`, animations via `gif`.

use std::path::Path;
use std::sync::Arc;

use dialogue::{ImageAtom, ImageData, ImageFrame, ImageKind};

pub fn from_path(path: &Path, kind: ImageKind) -> Result<ImageAtom, String> {
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("图片")
        .to_owned();
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let data = if ext == "gif" {
        decode_gif(&bytes)?
    } else {
        decode_still(&bytes)?
    };
    Ok(ImageAtom {
        kind,
        label,
        height_em: 1.6,
        data: Arc::new(data),
    })
}

pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>, label: impl Into<String>) -> ImageAtom {
    ImageAtom {
        kind: ImageKind::Clipboard,
        label: label.into(),
        height_em: 1.6,
        data: Arc::new(ImageData {
            width,
            height,
            frames: vec![ImageFrame { rgba, delay_ms: 0 }],
        }),
    }
}

fn decode_still(bytes: &[u8]) -> Result<ImageData, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    Ok(ImageData {
        width: image.width(),
        height: image.height(),
        frames: vec![ImageFrame {
            rgba: image.into_raw(),
            delay_ms: 0,
        }],
    })
}

fn decode_gif(bytes: &[u8]) -> Result<ImageData, String> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(std::io::Cursor::new(bytes))
        .map_err(|error| error.to_string())?;
    let width = u32::from(decoder.width());
    let height = u32::from(decoder.height());
    let mut canvas = vec![0u8; (width * height * 4) as usize];
    let mut frames = Vec::new();
    while let Some(frame) = decoder
        .read_next_frame()
        .map_err(|error| error.to_string())?
    {
        let left = u32::from(frame.left);
        let top = u32::from(frame.top);
        let fw = u32::from(frame.width);
        let fh = u32::from(frame.height);
        for row in 0..fh {
            for col in 0..fw {
                let src = ((row * fw + col) * 4) as usize;
                let x = left + col;
                let y = top + row;
                if x >= width || y >= height || src + 3 >= frame.buffer.len() {
                    continue;
                }
                let dst = ((y * width + x) * 4) as usize;
                let alpha = frame.buffer[src + 3];
                if alpha == 0 {
                    continue;
                }
                canvas[dst..dst + 4].copy_from_slice(&frame.buffer[src..src + 4]);
            }
        }
        let delay_ms = u32::from(frame.delay).saturating_mul(10).max(20);
        frames.push(ImageFrame {
            rgba: canvas.clone(),
            delay_ms,
        });
    }
    if frames.is_empty() {
        return decode_still(bytes);
    }
    Ok(ImageData {
        width,
        height,
        frames,
    })
}
