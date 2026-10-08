//! The active skin's sounds, decoded off the UI thread and handed to the
//! mixer. Skins without typing sounds get a synthesized set of soft clicks.

use std::sync::Arc;

use nanatype_audio::{AudioHandle, Bus, Pcm, SoundId, Voice, decode};
use nanatype_core::DEFAULT_TYPING_VARIANTS;
use nanatype_playback::CueKind;
use nanatype_theme::LoadedSkin;

pub type Bank = Vec<(SoundId, Arc<Pcm>)>;

pub fn typing_id(variant: u8) -> SoundId {
    format!("typing:{variant}").into()
}

pub fn se_id(name: &str) -> SoundId {
    format!("se:{name}").into()
}

pub const OPEN: &str = "ui:open";
pub const ADVANCE: &str = "ui:advance";

/// A short, soft key click: a decaying noise burst through a resonant tone.
fn synth_click(variant: u8) -> Pcm {
    let rate = 48_000u32;
    let len = (rate as f32 * 0.045) as usize;
    let tone = [1_650.0f32, 1_900.0, 1_420.0][usize::from(variant) % 3];
    let mut seed = 0x1234_5678u32 ^ u32::from(variant).wrapping_mul(0x9E37_79B9);
    let mut samples = Vec::with_capacity(len);
    let mut lp = 0.0f32;
    for i in 0..len {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let t = i as f32 / rate as f32;
        let env = (-t * 140.0).exp();
        lp += 0.35 * (noise - lp);
        let body = (t * tone * std::f32::consts::TAU).sin() * (-t * 90.0).exp();
        samples.push((lp * 0.55 + body * 0.45) * env * 0.5);
    }
    Pcm {
        rate,
        channels: 1,
        samples,
    }
}

/// Decode every sound of `skin` (runs on a worker thread).
pub fn load(skin: &LoadedSkin) -> (Bank, Vec<String>) {
    let mut bank: Bank = Vec::new();
    let mut errors = Vec::new();
    let mut read = |path: &str| -> Option<Arc<Pcm>> {
        let ext = path.rsplit('.').next().map(str::to_owned);
        match skin.files.read(path) {
            Ok(bytes) => match decode(bytes, ext.as_deref()) {
                Ok(pcm) => Some(Arc::new(pcm)),
                Err(e) => {
                    errors.push(format!("{path}: {e}"));
                    None
                }
            },
            Err(e) => {
                errors.push(format!("{path}: {e}"));
                None
            }
        }
    };
    let sounds = &skin.skin.sounds;
    if sounds.typing.files.is_empty() {
        for v in 0..DEFAULT_TYPING_VARIANTS {
            bank.push((typing_id(v), Arc::new(synth_click(v))));
        }
    } else {
        for (i, file) in sounds.typing.files.iter().enumerate() {
            if let Some(pcm) = read(file) {
                bank.push((typing_id(i as u8), pcm));
            }
        }
    }
    for (name, file) in &sounds.se {
        if let Some(pcm) = read(file) {
            bank.push((se_id(name), pcm));
        }
    }
    if let Some(file) = &sounds.open
        && let Some(pcm) = read(file)
    {
        bank.push((OPEN.into(), pcm));
    }
    if let Some(file) = &sounds.advance
        && let Some(pcm) = read(file)
    {
        bank.push((ADVANCE.into(), pcm));
    }
    (bank, errors)
}

pub fn install(audio: &AudioHandle, previous: &Bank, next: &Bank) {
    for (id, _) in previous {
        if !next.iter().any(|(n, _)| n == id) {
            audio.unload(id.clone());
        }
    }
    for (id, pcm) in next {
        audio.load(id.clone(), pcm.clone());
    }
}

/// The voice a cue plays, if it plays one.
pub fn voice(cue: &CueKind, skin: &LoadedSkin) -> Option<Voice> {
    match cue {
        CueKind::TypeSound { variant, seed, .. } => {
            let t = &skin.skin.sounds.typing;
            let r = (seed.wrapping_mul(0x2545_F491) >> 8) as f32 / (1u32 << 24) as f32;
            Some(Voice {
                sound: typing_id(*variant),
                bus: Bus::Typing,
                gain: if t.files.is_empty() { 0.8 } else { t.volume },
                pitch: 1.0 + t.pitch_jitter * (r * 2.0 - 1.0),
            })
        }
        CueKind::Sound { sound, gain } => Some(Voice {
            sound: se_id(sound),
            bus: Bus::Effects,
            gain: *gain,
            pitch: 1.0,
        }),
        _ => None,
    }
}
