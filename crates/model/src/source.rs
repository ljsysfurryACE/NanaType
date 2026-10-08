//! Message sources feeding the history list.
//!
//! Today the only source is the local typewriter. Live-platform chat (for
//! example Bilibili danmaku) plugs in later as another [`MessageSource`]
//! without the history view changing.

use serde::{Deserialize, Serialize};

use crate::{Document, LineId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceId(pub String);

impl SourceId {
    pub const LOCAL: &'static str = "local";

    pub fn local() -> Self {
        Self(Self::LOCAL.to_owned())
    }

    pub fn is_local(&self) -> bool {
        self.0 == Self::LOCAL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Lines the streamer typed.
    Typewriter,
    /// Viewer chat from a live platform.
    Chat,
    /// Gifts, memberships and other paid events.
    Event,
}

/// One row of the history list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub source: SourceId,
    pub kind: SourceKind,
    /// Present for typewriter lines, so the row can be recalled into the editor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineId>,
    pub author: String,
    /// Avatar reference understood by the source (asset ref or URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    pub doc: Document,
    /// Unix milliseconds.
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceEvent {
    Item(HistoryItem),
    /// The source connected, reconnected or failed; `detail` is for the UI.
    Status {
        connected: bool,
        detail: String,
    },
}

/// A producer of history rows. Implementations run on their own thread and
/// deliver through `emit`, which wakes the UI; they never touch UI state.
pub trait MessageSource: Send {
    fn id(&self) -> SourceId;
    fn kind(&self) -> SourceKind;
    /// Begin producing. `emit` may be called from any thread.
    fn start(&mut self, emit: Box<dyn Fn(SourceEvent) + Send + Sync>);
    fn stop(&mut self);
}
