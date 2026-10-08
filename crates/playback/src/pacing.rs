use nanatype_model::{CharClass, Pace, PunctKind};
use serde::{Deserialize, Serialize};

/// Extra pause after punctuation, on top of the base delay.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PunctPauses {
    pub comma_ms: u16,
    pub stop_ms: u16,
    pub ellipsis_ms: u16,
}

impl Default for PunctPauses {
    fn default() -> Self {
        Self {
            comma_ms: 120,
            stop_ms: 260,
            ellipsis_ms: 300,
        }
    }
}

/// How fast characters reveal. The skin provides defaults; settings scale them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PacingParams {
    /// Delay after a Latin character.
    pub base_ms: u16,
    /// CJK characters wait this many base delays (two beats by default).
    pub cjk_factor: f32,
    pub space_ms: u16,
    pub punct: PunctPauses,
    pub punctuation_pauses: bool,
    /// Global multiplier from settings; `0.5` is twice as fast.
    pub speed_scale: f32,
    /// `false` shows each segment at once (typing simulation off).
    pub typing: bool,
}

impl Default for PacingParams {
    fn default() -> Self {
        Self {
            base_ms: 50,
            cjk_factor: 2.0,
            space_ms: 50,
            punct: PunctPauses::default(),
            punctuation_pauses: true,
            speed_scale: 1.0,
            typing: true,
        }
    }
}

impl PacingParams {
    /// Delay after revealing a unit of `class` inside a span paced by `pace`.
    pub fn delay_ms(&self, class: CharClass, pace: Pace) -> u32 {
        if !self.typing {
            return 0;
        }
        let base = f32::from(self.base_ms.max(1));
        let beat = match class {
            CharClass::Cjk => base * self.cjk_factor.max(0.0),
            CharClass::Latin | CharClass::Atom => base,
            CharClass::Space => f32::from(self.space_ms),
            CharClass::Punct(kind) => {
                let extra = if self.punctuation_pauses {
                    match kind {
                        PunctKind::Comma => self.punct.comma_ms,
                        PunctKind::Stop => self.punct.stop_ms,
                        PunctKind::Ellipsis => self.punct.ellipsis_ms,
                        PunctKind::Other => 0,
                    }
                } else {
                    0
                };
                base + f32::from(extra)
            }
        };
        let ms = match pace {
            Pace::Inherit => beat * self.speed_scale.max(0.0),
            Pace::Scale(scale) => beat * scale.max(0.0) * self.speed_scale.max(0.0),
            Pace::FixedMs(ms) => f32::from(ms),
            Pace::Instant => 0.0,
        };
        ms.round().clamp(0.0, 60_000.0) as u32
    }
}

/// Which revealed units click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TypingSoundRule {
    pub enabled: bool,
    /// Click on every n-th eligible unit.
    pub every_n: u8,
    pub skip_space: bool,
    pub skip_punct: bool,
    /// Two clicks closer than this collapse into one (instant spans).
    pub min_interval_ms: u16,
    /// Number of sample variants to rotate through.
    pub variants: u8,
}

impl Default for TypingSoundRule {
    fn default() -> Self {
        Self {
            enabled: true,
            every_n: 1,
            skip_space: true,
            skip_punct: true,
            min_interval_ms: 30,
            variants: 1,
        }
    }
}
