use std::sync::Arc;

use nanatype_model::{AfterLine, LineId};

use crate::{Cue, SegmentEnd, Timeline};

/// Length of the fade-out once a fading line's hold time is over.
pub const FADE_OUT_MS: u32 = 450;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeReason {
    SegmentEnd,
    AutoContinue,
    AfterLine,
    FadeDone,
    AutoAdvance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Revealing,
    /// Waiting at a pause or page marker for the continue key.
    Gated,
    /// Fully revealed; holding or counting down to a fade.
    Done,
    Fading,
    Finished,
}

/// Instructions for the host. Each is idempotent for its `epoch`; a newer
/// epoch makes every earlier wake and cue stale.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEffect {
    /// Animate segment `seg` from `origin_ms`. Units before `segment.units`
    /// are already visible; units from `visible_from` on are drawn.
    ShowSegment {
        epoch: u64,
        seg: usize,
        origin_ms: u64,
        /// Reveal offsets; all zero when the segment was skipped.
        reveal_ms: Arc<[u32]>,
    },
    ScheduleCues {
        epoch: u64,
        origin_ms: u64,
        cues: Arc<[Cue]>,
    },
    /// Fire these cues right now (skip kept expressions and sounds).
    FireNow {
        cues: Vec<Cue>,
    },
    WakeAt {
        epoch: u64,
        at_ms: u64,
        reason: WakeReason,
    },
    ContinueIndicator(bool),
    /// Start the fade-out animation.
    FadeOut {
        at_ms: u64,
        ms: u32,
    },
    /// Every unit of the line is visible.
    LineRevealed(LineId),
    /// The line left the box (fade or clear finished).
    LineFinished(LineId),
    /// `auto_advance_ms` elapsed: start the next queued line if any.
    AutoAdvance(LineId),
}

/// What is on screen at a moment, for tests and for history sync.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RevealSnapshot {
    pub phase: Phase,
    pub visible_from: u32,
    /// Units `visible_from..revealed` are visible.
    pub revealed: u32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Revealing {
        seg: usize,
        origin: u64,
        skipped: bool,
    },
    Gated {
        seg: usize,
    },
    Done {
        at: u64,
    },
    Fading {
        at: u64,
        ms: u32,
    },
    Finished,
}

pub struct Player {
    line: LineId,
    timeline: Arc<Timeline>,
    after: AfterLine,
    auto_advance_ms: Option<u32>,
    state: State,
    epoch: u64,
}

impl Player {
    pub fn start(
        line: LineId,
        timeline: Arc<Timeline>,
        after: AfterLine,
        auto_advance_ms: Option<u32>,
        now: u64,
    ) -> (Self, Vec<PlayerEffect>) {
        let mut player = Self {
            line,
            timeline,
            after,
            auto_advance_ms,
            state: State::Finished,
            epoch: 0,
        };
        let mut out = Vec::new();
        player.enter_segment(0, now, &mut out);
        (player, out)
    }

    pub fn line(&self) -> LineId {
        self.line
    }

    pub fn timeline(&self) -> &Arc<Timeline> {
        &self.timeline
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn phase(&self) -> Phase {
        match self.state {
            State::Revealing { .. } => Phase::Revealing,
            State::Gated { .. } => Phase::Gated,
            State::Done { .. } => Phase::Done,
            State::Fading { .. } => Phase::Fading,
            State::Finished => Phase::Finished,
        }
    }

    /// Current segment index (the last one once done).
    pub fn segment(&self) -> usize {
        match self.state {
            State::Revealing { seg, .. } | State::Gated { seg } => seg,
            _ => self.timeline.last_segment(),
        }
    }

    fn bump(&mut self) -> u64 {
        self.epoch += 1;
        self.epoch
    }

    fn enter_segment(&mut self, seg: usize, now: u64, out: &mut Vec<PlayerEffect>) {
        let epoch = self.bump();
        let segment = &self.timeline.segments[seg];
        self.state = State::Revealing {
            seg,
            origin: now,
            skipped: false,
        };
        out.push(PlayerEffect::ContinueIndicator(false));
        out.push(PlayerEffect::ShowSegment {
            epoch,
            seg,
            origin_ms: now,
            reveal_ms: segment.reveal_ms.clone(),
        });
        if !segment.cues.is_empty() {
            out.push(PlayerEffect::ScheduleCues {
                epoch,
                origin_ms: now,
                cues: segment.cues.clone(),
            });
        }
        out.push(PlayerEffect::WakeAt {
            epoch,
            at_ms: now + u64::from(segment.duration_ms),
            reason: WakeReason::SegmentEnd,
        });
    }

    fn end_segment(&mut self, seg: usize, now: u64, out: &mut Vec<PlayerEffect>) {
        let end = self.timeline.segments[seg].end;
        match end {
            SegmentEnd::Pause { auto_ms } => {
                let epoch = self.bump();
                self.state = State::Gated { seg };
                out.push(PlayerEffect::ContinueIndicator(true));
                if let Some(ms) = auto_ms {
                    out.push(PlayerEffect::WakeAt {
                        epoch,
                        at_ms: now + u64::from(ms),
                        reason: WakeReason::AutoContinue,
                    });
                }
            }
            SegmentEnd::Page => {
                self.bump();
                self.state = State::Gated { seg };
                out.push(PlayerEffect::ContinueIndicator(true));
            }
            SegmentEnd::LineEnd => self.enter_done(now, out),
        }
    }

    fn enter_done(&mut self, now: u64, out: &mut Vec<PlayerEffect>) {
        let epoch = self.bump();
        self.state = State::Done { at: now };
        out.push(PlayerEffect::ContinueIndicator(false));
        out.push(PlayerEffect::LineRevealed(self.line));
        let units = self.timeline.unit_count;
        match self.after {
            AfterLine::Hold => {}
            AfterLine::Fade {
                delay_ms,
                per_unit_ms,
            } => out.push(PlayerEffect::WakeAt {
                epoch,
                at_ms: now + u64::from(delay_ms) + u64::from(per_unit_ms) * u64::from(units),
                reason: WakeReason::AfterLine,
            }),
            AfterLine::Clear { delay_ms } => out.push(PlayerEffect::WakeAt {
                epoch,
                at_ms: now + u64::from(delay_ms),
                reason: WakeReason::AfterLine,
            }),
        }
        if let Some(ms) = self.auto_advance_ms {
            out.push(PlayerEffect::WakeAt {
                epoch,
                at_ms: now + u64::from(ms),
                reason: WakeReason::AutoAdvance,
            });
        }
    }

    /// A scheduled wake arrived. Stale epochs are ignored.
    pub fn on_wake(&mut self, epoch: u64, reason: WakeReason, now: u64) -> Vec<PlayerEffect> {
        let mut out = Vec::new();
        if epoch != self.epoch {
            return out;
        }
        match (self.state, reason) {
            (State::Revealing { seg, .. }, WakeReason::SegmentEnd) => {
                self.end_segment(seg, now, &mut out)
            }
            (State::Gated { seg }, WakeReason::AutoContinue) => {
                self.enter_segment(seg + 1, now, &mut out)
            }
            (State::Done { .. }, WakeReason::AfterLine) => match self.after {
                AfterLine::Fade { .. } => {
                    let epoch = self.bump();
                    self.state = State::Fading {
                        at: now,
                        ms: FADE_OUT_MS,
                    };
                    out.push(PlayerEffect::FadeOut {
                        at_ms: now,
                        ms: FADE_OUT_MS,
                    });
                    out.push(PlayerEffect::WakeAt {
                        epoch,
                        at_ms: now + u64::from(FADE_OUT_MS),
                        reason: WakeReason::FadeDone,
                    });
                }
                _ => self.finish(&mut out),
            },
            (State::Done { .. }, WakeReason::AutoAdvance) => {
                out.push(PlayerEffect::AutoAdvance(self.line))
            }
            (State::Fading { .. }, WakeReason::FadeDone) => self.finish(&mut out),
            _ => {}
        }
        out
    }

    fn finish(&mut self, out: &mut Vec<PlayerEffect>) {
        self.bump();
        self.state = State::Finished;
        out.push(PlayerEffect::LineFinished(self.line));
    }

    /// The continue key. While revealing it completes the segment; at a gate
    /// it starts the next segment. Returns `true` when it did something.
    pub fn advance(&mut self, now: u64) -> (bool, Vec<PlayerEffect>) {
        let mut out = Vec::new();
        match self.state {
            State::Revealing { seg, origin, .. } => {
                self.skip_segment(seg, origin, now, &mut out);
                (true, out)
            }
            State::Gated { seg } => {
                self.enter_segment(seg + 1, now, &mut out);
                (true, out)
            }
            _ => (false, out),
        }
    }

    /// Reveal the rest of the current segment at once. Pending sounds and
    /// expressions fire now; typing clicks and shakes are dropped.
    fn skip_segment(&mut self, seg: usize, origin: u64, now: u64, out: &mut Vec<PlayerEffect>) {
        let epoch = self.bump();
        let segment = &self.timeline.segments[seg];
        let elapsed = now.saturating_sub(origin);
        let pending: Vec<Cue> = segment
            .cues
            .iter()
            .filter(|c| u64::from(c.at_ms) > elapsed && c.kind.survives_skip())
            .cloned()
            .collect();
        let zeros: Arc<[u32]> = vec![0; segment.reveal_ms.len()].into();
        self.state = State::Revealing {
            seg,
            origin: now,
            skipped: true,
        };
        out.push(PlayerEffect::ShowSegment {
            epoch,
            seg,
            origin_ms: now,
            reveal_ms: zeros,
        });
        if !pending.is_empty() {
            out.push(PlayerEffect::FireNow { cues: pending });
        }
        self.end_segment(seg, now, out);
    }

    /// Jump to the fully revealed end of the line (queue "finish then next").
    pub fn finish_reveal(&mut self, now: u64) -> Vec<PlayerEffect> {
        let mut out = Vec::new();
        let last = self.timeline.last_segment();
        let epoch = self.bump();
        out.push(PlayerEffect::ShowSegment {
            epoch,
            seg: last,
            origin_ms: now,
            reveal_ms: vec![0; self.timeline.segments[last].reveal_ms.len()].into(),
        });
        self.enter_done(now, &mut out);
        out
    }

    pub fn snapshot(&self, now: u64) -> RevealSnapshot {
        let tl = &self.timeline;
        let (seg, revealed) = match self.state {
            State::Revealing {
                seg,
                origin,
                skipped,
            } => {
                let s = &tl.segments[seg];
                let elapsed = now.saturating_sub(origin);
                let shown = if skipped {
                    s.reveal_ms.len()
                } else {
                    s.reveal_ms
                        .iter()
                        .filter(|t| u64::from(**t) <= elapsed)
                        .count()
                };
                (seg, s.units.start + shown as u32)
            }
            State::Gated { seg } => (seg, tl.segments[seg].units.end),
            _ => (tl.last_segment(), tl.unit_count),
        };
        let opacity = match self.state {
            State::Fading { at, ms } => {
                1.0 - (now.saturating_sub(at) as f32 / ms.max(1) as f32).clamp(0.0, 1.0)
            }
            State::Finished => 0.0,
            _ => 1.0,
        };
        RevealSnapshot {
            phase: self.phase(),
            visible_from: tl.segments[seg].visible_from,
            revealed,
            opacity,
        }
    }
}
