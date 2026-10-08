use std::sync::Arc;

use nanatype_model::{Document, Entrance, HistoryItem, LineId, Speaker};

use crate::Millis;

/// Units of the current segment that animate in.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentReveal {
    pub origin_ms: Millis,
    /// First unit of the segment.
    pub start: u32,
    /// Reveal offset of each unit from `origin_ms`.
    pub reveal_ms: Arc<[u32]>,
}

/// Which units of a document a surface draws, and when each appears.
#[derive(Debug, Clone, PartialEq)]
pub struct Reveal {
    /// Units before this are off the page (cleared by a page marker).
    pub visible_from: u32,
    /// Units `visible_from..shown` are fully visible.
    pub shown: u32,
    pub segment: Option<SegmentReveal>,
    /// Units at or after this stay hidden.
    pub hidden_from: u32,
}

impl Reveal {
    pub fn full(units: u32) -> Self {
        Self {
            visible_from: 0,
            shown: units,
            segment: None,
            hidden_from: units,
        }
    }

    pub fn is_full(&self, units: u32) -> bool {
        self.visible_from == 0 && self.shown >= units && self.segment.is_none()
    }

    /// Reveal time of `unit` on the host clock: `Some(0)` for units already
    /// shown, `None` for hidden or paged-out units.
    pub fn unit_at(&self, unit: u32) -> Option<Millis> {
        if unit < self.visible_from || unit >= self.hidden_from {
            return None;
        }
        if unit < self.shown {
            return Some(0);
        }
        let Some(seg) = self.segment.as_ref() else {
            return Some(0);
        };
        let i = unit.checked_sub(seg.start)? as usize;
        // Units past the animated segment but before `hidden_from` belong to an
        // interrupted line and are simply shown.
        Some(
            seg.reveal_ms
                .get(i)
                .map_or(0, |offset| seg.origin_ms + u64::from(*offset)),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShakeView {
    pub at_ms: Millis,
    pub amp: f32,
    pub ms: u32,
}

/// Everything the typewriter (and the input preview) needs to paint.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayView {
    /// Bumps on every change; renderers compare it to skip work.
    pub revision: u64,
    pub line: Option<LineId>,
    /// The line, markers included (they never paint, but keep unit indices).
    pub doc: Arc<Document>,
    pub speaker: Speaker,
    pub reveal: Reveal,
    /// Active portrait variant.
    pub portrait: Option<String>,
    pub continue_indicator: bool,
    /// Fade-out start and duration.
    pub fade: Option<(Millis, u32)>,
    pub shake: Option<ShakeView>,
    pub entrance: Entrance,
    /// Nothing is on screen (the box itself is hidden).
    pub empty: bool,
}

impl DisplayView {
    pub fn empty(entrance: Entrance) -> Self {
        Self {
            revision: 0,
            line: None,
            doc: Arc::new(Document::new()),
            speaker: Speaker::default(),
            reveal: Reveal::full(0),
            portrait: None,
            continue_indicator: false,
            fade: None,
            shake: None,
            entrance,
            empty: true,
        }
    }
}

/// One row of the history list.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRow {
    /// Stable key for list diffing and entrance animation.
    pub key: u64,
    pub item: HistoryItem,
    /// The row's document without markers (unit indices still match the line,
    /// since markers are not units).
    pub doc: Arc<Document>,
    /// `None` is fully shown; `Some` types along with the display.
    pub reveal: Option<Reveal>,
}
