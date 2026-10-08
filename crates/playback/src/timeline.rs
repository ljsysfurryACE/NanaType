use std::ops::Range;
use std::sync::Arc;

use nanatype_model::{CharClass, Document, Item, Marker};
use serde::{Deserialize, Serialize};

use crate::{PacingParams, TypingSoundRule};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cue", rename_all = "snake_case")]
pub enum CueKind {
    TypeSound {
        unit: u32,
        variant: u8,
        seed: u32,
    },
    Sound {
        sound: String,
        gain: f32,
    },
    Expression {
        name: String,
        intensity: Option<f32>,
    },
    Portrait {
        variant: String,
    },
    Shake {
        amp: f32,
        ms: u32,
    },
    /// The speaker starts or stops "talking" (mouth flap for avatars).
    SpeakingStart,
    SpeakingStop,
}

impl CueKind {
    /// Cues that still matter when the reveal is skipped.
    pub fn survives_skip(&self) -> bool {
        matches!(
            self,
            Self::Sound { .. } | Self::Expression { .. } | Self::Portrait { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    /// Offset from the segment origin.
    pub at_ms: u32,
    pub kind: CueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentEnd {
    /// Wait for continue; continue on its own after `auto_ms`.
    Pause {
        auto_ms: Option<u32>,
    },
    /// Wait for continue, then clear the box.
    Page,
    LineEnd,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    /// Units this segment reveals.
    pub units: Range<u32>,
    /// First unit that is visible while this segment plays (page start).
    pub visible_from: u32,
    /// Reveal offset per unit in `units`, from the segment origin.
    pub reveal_ms: Arc<[u32]>,
    pub cues: Arc<[Cue]>,
    /// When the segment's reveal (and trailing waits) are over.
    pub duration_ms: u32,
    pub end: SegmentEnd,
}

/// A compiled line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub unit_count: u32,
    pub segments: Vec<Segment>,
}

impl Timeline {
    pub fn last_segment(&self) -> usize {
        self.segments.len().saturating_sub(1)
    }
}

struct Draft {
    units: Vec<u32>,
    cues: Vec<Cue>,
    start: u32,
    visible_from: u32,
    clock: u32,
    speaking: bool,
}

/// Compile `doc` into segments. Pure; call again when pacing settings change.
pub fn compile(doc: &Document, pacing: &PacingParams, sound: &TypingSoundRule) -> Timeline {
    let mut segments = Vec::new();
    let mut ordinal: u32 = 0;
    let mut eligible: u32 = 0;
    let mut last_click: Option<u32> = None;
    let mut variant: u8 = 0;
    let mut d = Draft {
        units: Vec::new(),
        cues: Vec::new(),
        start: 0,
        visible_from: 0,
        clock: 0,
        speaking: false,
    };

    fn close(d: &mut Draft, end: SegmentEnd, ordinal: u32, segments: &mut Vec<Segment>) {
        if d.speaking {
            d.cues.push(Cue {
                at_ms: d.clock,
                kind: CueKind::SpeakingStop,
            });
            d.speaking = false;
        }
        d.cues.sort_by_key(|c| c.at_ms);
        segments.push(Segment {
            units: d.start..ordinal,
            visible_from: d.visible_from,
            reveal_ms: std::mem::take(&mut d.units).into(),
            cues: std::mem::take(&mut d.cues).into(),
            duration_ms: d.clock,
            end,
        });
        d.start = ordinal;
        d.clock = 0;
    }

    let mut pending_delay = 0u32;
    doc.walk(|item| match item {
        Item::Unit(unit) => {
            d.clock = d.clock.saturating_add(pending_delay);
            let class = unit.class();
            if !d.speaking && !matches!(class, CharClass::Space) {
                d.cues.push(Cue {
                    at_ms: d.clock,
                    kind: CueKind::SpeakingStart,
                });
                d.speaking = true;
            }
            d.units.push(d.clock);
            let clicks = sound.enabled
                && !(sound.skip_space && class == CharClass::Space)
                && !(sound.skip_punct && matches!(class, CharClass::Punct(_)));
            if clicks {
                let due = eligible.is_multiple_of(u32::from(sound.every_n.max(1)));
                eligible += 1;
                let spaced = last_click
                    .is_none_or(|t| d.clock.saturating_sub(t) >= u32::from(sound.min_interval_ms));
                if due && spaced {
                    d.cues.push(Cue {
                        at_ms: d.clock,
                        kind: CueKind::TypeSound {
                            unit: ordinal,
                            variant,
                            seed: ordinal.wrapping_mul(0x9E37_79B9),
                        },
                    });
                    variant = (variant + 1) % sound.variants.max(1);
                    last_click = Some(d.clock);
                }
            }
            pending_delay = pacing.delay_ms(class, unit.pace);
            ordinal += 1;
        }
        Item::Marker(marker) => {
            let at = d.clock.saturating_add(pending_delay);
            match marker {
                Marker::Wait { ms } => {
                    pending_delay =
                        pending_delay.saturating_add(if pacing.typing { *ms } else { 0 });
                }
                Marker::Pause { auto_ms } => {
                    d.clock = at;
                    pending_delay = 0;
                    close(
                        &mut d,
                        SegmentEnd::Pause { auto_ms: *auto_ms },
                        ordinal,
                        &mut segments,
                    );
                    last_click = None;
                }
                Marker::Page => {
                    d.clock = at;
                    pending_delay = 0;
                    close(&mut d, SegmentEnd::Page, ordinal, &mut segments);
                    d.visible_from = ordinal;
                    last_click = None;
                }
                Marker::Sound { sound, gain } => d.cues.push(Cue {
                    at_ms: at,
                    kind: CueKind::Sound {
                        sound: sound.clone(),
                        gain: *gain,
                    },
                }),
                Marker::Expression { name, intensity } => d.cues.push(Cue {
                    at_ms: at,
                    kind: CueKind::Expression {
                        name: name.clone(),
                        intensity: *intensity,
                    },
                }),
                Marker::Portrait { variant } => d.cues.push(Cue {
                    at_ms: at,
                    kind: CueKind::Portrait {
                        variant: variant.clone(),
                    },
                }),
                Marker::ScreenShake { amp, ms } => d.cues.push(Cue {
                    at_ms: at,
                    kind: CueKind::Shake { amp: *amp, ms: *ms },
                }),
            }
        }
    });
    d.clock = d.clock.saturating_add(pending_delay);
    close(&mut d, SegmentEnd::LineEnd, ordinal, &mut segments);
    Timeline {
        unit_count: ordinal,
        segments,
    }
}
