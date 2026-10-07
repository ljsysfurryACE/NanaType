//! Submit-and-replace playback.
//!
//! `Revealing` walks the piece list. `Pause` waits for the continue key.
//! After the last piece the line either holds or fades. The clock is a
//! millisecond counter so tests do not sleep.

use serde::{Deserialize, Serialize};

use crate::document::{DialogueDocument, Piece};
use crate::theme::Motion;
use crate::{DEFAULT_SPEED_MS, POP_MS, is_cjk_grapheme};

/// What happens after the last grapheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AfterLine {
    #[default]
    Hold,
    Fade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Revealing,
    WaitingForKey,
    Holding,
    Fading,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackConfig {
    pub speed_ms: u32,
    pub simulate_typing: bool,
    pub simulate_pop: bool,
    pub after: AfterLine,
    pub fade_base_ms: u32,
    pub fade_per_char_ms: u32,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            speed_ms: DEFAULT_SPEED_MS,
            simulate_typing: true,
            simulate_pop: true,
            after: AfterLine::Hold,
            fade_base_ms: 1_200,
            fade_per_char_ms: 40,
        }
    }
}

pub struct Playback {
    document: DialogueDocument,
    name: String,
    cursor: usize,
    speed_ms: u32,
    phase: Phase,
    instant: bool,
    next_due: u64,
    started: u64,
    fade_started: u64,
    fade_duration: u32,
    config: PlaybackConfig,
}

impl Playback {
    /// Begin a submitted sentence at `now_ms`. The first step is due immediately;
    /// later graphemes wait [`PlaybackConfig::speed_ms`] (CJK waits two beats).
    pub fn start(
        document: DialogueDocument,
        name: impl Into<String>,
        config: PlaybackConfig,
        now_ms: u64,
    ) -> Self {
        let mut play = Self {
            document,
            name: name.into(),
            cursor: 0,
            speed_ms: config.speed_ms.max(1),
            phase: Phase::Revealing,
            instant: false,
            next_due: now_ms,
            started: now_ms,
            fade_started: now_ms,
            fade_duration: 0,
            config,
        };
        if !play.config.simulate_typing {
            play.cursor = play.document.len();
            play.enter_after(now_ms);
        }
        play
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn document(&self) -> &DialogueDocument {
        &self.document
    }

    /// Pieces the typewriter may draw, including markers already passed.
    /// The painter skips markers; they are not chips on this surface.
    pub fn revealed(&self) -> &[Piece] {
        &self.document.pieces()[..self.cursor.min(self.document.len())]
    }

    pub fn pop_enabled(&self) -> bool {
        self.config.simulate_pop
    }

    /// Advance the clock. Returns how many graphemes or images became visible
    /// on this call, for the typewriter click.
    pub fn tick(&mut self, now_ms: u64) -> u32 {
        if self.phase == Phase::Fading {
            let elapsed = now_ms.saturating_sub(self.fade_started);
            if elapsed >= u64::from(self.fade_duration) {
                self.phase = Phase::Finished;
            }
            return 0;
        }
        if self.phase != Phase::Revealing {
            return 0;
        }
        let mut shown = 0u32;
        for _ in 0..8_192 {
            if self.cursor >= self.document.len() {
                self.enter_after(now_ms);
                break;
            }
            if now_ms < self.next_due && !self.instant {
                break;
            }
            match &self.document.pieces()[self.cursor] {
                Piece::Wait { ms } => {
                    let wait = u64::from(*ms);
                    self.cursor += 1;
                    self.instant = false;
                    self.next_due = now_ms.saturating_add(wait);
                    break;
                }
                Piece::Speed { ms_per_char } => {
                    self.speed_ms = (*ms_per_char).max(1);
                    self.cursor += 1;
                    self.instant = false;
                    self.next_due = now_ms.saturating_add(u64::from(self.speed_ms));
                    break;
                }
                Piece::Instant => {
                    self.cursor += 1;
                    self.instant = true;
                }
                Piece::Pause => {
                    self.cursor += 1;
                    self.instant = false;
                    self.phase = Phase::WaitingForKey;
                    break;
                }
                Piece::Grapheme { cluster, .. } => {
                    let cjk = is_cjk_grapheme(cluster);
                    self.cursor += 1;
                    shown += 1;
                    if !self.instant {
                        let beats = if cjk { 2 } else { 1 };
                        self.next_due =
                            now_ms.saturating_add(u64::from(self.speed_ms.saturating_mul(beats)));
                        break;
                    }
                }
                Piece::Image(_) => {
                    self.cursor += 1;
                    shown += 1;
                    if !self.instant {
                        self.next_due = now_ms.saturating_add(u64::from(self.speed_ms));
                        break;
                    }
                }
            }
        }
        shown
    }

    /// The continue hotkey. It never submits a draft.
    pub fn continue_key(&mut self, now_ms: u64) -> bool {
        if self.phase != Phase::WaitingForKey {
            return false;
        }
        self.phase = Phase::Revealing;
        self.next_due = now_ms;
        self.instant = false;
        true
    }

    /// Whether the typewriter picture still changes on its own.
    /// A held line with no motion keeps the last presented frame.
    pub fn wants_frame(&self, now_ms: u64) -> bool {
        match self.phase {
            Phase::Revealing | Phase::Fading => true,
            Phase::Finished => false,
            Phase::WaitingForKey | Phase::Holding => {
                pop_progress(
                    now_ms.saturating_sub(self.started),
                    self.config.simulate_pop,
                ) < 1.0
                    || self.revealed().iter().any(|piece| match piece {
                        Piece::Grapheme { style, .. } => style.motion != Motion::None,
                        Piece::Image(image) => image.data.frames.len() > 1,
                        _ => false,
                    })
            }
        }
    }

    pub fn opacity(&self, now_ms: u64) -> f32 {
        let pop = pop_progress(
            now_ms.saturating_sub(self.started),
            self.config.simulate_pop,
        );
        let fade = match self.phase {
            Phase::Fading => {
                let duration = u64::from(self.fade_duration.max(1));
                let t = now_ms.saturating_sub(self.fade_started) as f32 / duration as f32;
                (1.0 - t).clamp(0.0, 1.0)
            }
            Phase::Finished => 0.0,
            _ => 1.0,
        };
        pop * fade
    }

    pub fn scale(&self, now_ms: u64) -> f32 {
        pop_scale(pop_progress(
            now_ms.saturating_sub(self.started),
            self.config.simulate_pop,
        ))
    }

    fn enter_after(&mut self, now_ms: u64) {
        match self.config.after {
            AfterLine::Hold => self.phase = Phase::Holding,
            AfterLine::Fade => {
                let chars = self.document.grapheme_count() as u32;
                self.fade_duration = self
                    .config
                    .fade_base_ms
                    .saturating_add(self.config.fade_per_char_ms.saturating_mul(chars));
                self.fade_started = now_ms;
                if self.fade_duration == 0 {
                    self.phase = Phase::Finished;
                } else {
                    self.phase = Phase::Fading;
                }
            }
        }
    }
}

pub fn pop_progress(elapsed_ms: u64, enabled: bool) -> f32 {
    if !enabled {
        return 1.0;
    }
    (elapsed_ms as f32 / POP_MS as f32).clamp(0.0, 1.0)
}

pub fn pop_scale(progress: f32) -> f32 {
    0.92 + 0.08 * progress.clamp(0.0, 1.0)
}
