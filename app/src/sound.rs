//! Optional typewriter click.
//!
//! rodio 0.20 pulls cpal 0.15, which pins the `windows` crate at 0.54. That
//! splits `gpu-allocator` off the 0.62 copy wgpu-hal uses and the DX12
//! backend stops compiling. The click is a short in-memory wav played with
//! PlaySound so the transparent windows can still link.

use std::sync::OnceLock;

static WAV: OnceLock<Vec<u8>> = OnceLock::new();

pub fn click() {
    #[cfg(windows)]
    {
        let wav = WAV.get_or_init(click_wav);
        unsafe {
            use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
            use windows::core::PCWSTR;
            let _ = PlaySoundW(
                PCWSTR(wav.as_ptr() as *const u16),
                None,
                SND_ASYNC | SND_MEMORY | SND_NODEFAULT,
            );
        }
    }
}

#[cfg(windows)]
fn click_wav() -> Vec<u8> {
    let samples = 1_764u32;
    let data_bytes = samples * 2;
    let mut out = Vec::with_capacity(44 + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&44_100u32.to_le_bytes());
    out.extend_from_slice(&(44_100u32 * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for index in 0..samples {
        let t = index as f32 / 44_100.0;
        let env = (1.0 - t / 0.04).max(0.0);
        let wave = (t * 880.0 * std::f32::consts::TAU).sin();
        let sample = (wave * env * 12_000.0) as i16;
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}
