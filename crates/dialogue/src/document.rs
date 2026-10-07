//! Editable dialogue: graphemes, images and performance markers.
//!
//! The caret is an index into [`DialogueDocument::pieces`], `0..=len`. A
//! selection is the half-open range between the caret and an anchor. Undo
//! stores whole snapshots, including the pending typing style.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::theme::Motion;

const UNDO_CAP: usize = 64;

/// Colour and motion applied to one grapheme. `None` colour means the theme body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub color: Option<[u8; 4]>,
    pub size_px: Option<f32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub motion: Motion,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: None,
            size_px: None,
            bold: false,
            italic: false,
            underline: false,
            motion: Motion::None,
        }
    }
}

impl TextStyle {
    pub fn with_motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }
}

/// Where an image atom came from. Clipboard atoms keep their pixels in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageKind {
    File,
    Sticker,
    Clipboard,
}

/// One decoded frame. `rgba` is straight (not premultiplied) tightly packed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFrame {
    pub rgba: Vec<u8>,
    pub delay_ms: u32,
}

/// Shared pixels so undo snapshots do not copy the bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<ImageFrame>,
}

impl ImageData {
    pub fn frame_at(&self, elapsed_ms: u64) -> Option<&ImageFrame> {
        let frames = &self.frames;
        if frames.is_empty() || self.width == 0 || self.height == 0 {
            return None;
        }
        if frames.len() == 1 {
            return frames.first();
        }
        let total: u64 = frames
            .iter()
            .map(|frame| u64::from(frame.delay_ms.max(1)))
            .sum();
        if total == 0 {
            return frames.first();
        }
        let mut at = elapsed_ms % total;
        for frame in frames {
            let delay = u64::from(frame.delay_ms.max(1));
            if at < delay {
                return Some(frame);
            }
            at -= delay;
        }
        frames.last()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageAtom {
    pub kind: ImageKind,
    pub label: String,
    pub height_em: f32,
    pub data: Arc<ImageData>,
}

/// One caret step. Markers are edited as chips and executed only by playback.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Grapheme { cluster: String, style: TextStyle },
    Image(ImageAtom),
    Wait { ms: u32 },
    Speed { ms_per_char: u32 },
    Instant,
    Pause,
}

impl Piece {
    pub fn is_marker(&self) -> bool {
        matches!(
            self,
            Self::Wait { .. } | Self::Speed { .. } | Self::Instant | Self::Pause
        )
    }

    pub fn is_visible(&self) -> bool {
        matches!(self, Self::Grapheme { .. } | Self::Image(_))
    }
}

#[derive(Clone)]
struct Snapshot {
    pieces: Vec<Piece>,
    caret: usize,
    anchor: Option<usize>,
    pending: TextStyle,
}

/// The draft or a submitted sentence.
#[derive(Clone)]
pub struct DialogueDocument {
    pieces: Vec<Piece>,
    caret: usize,
    anchor: Option<usize>,
    pending: TextStyle,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Default for DialogueDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogueDocument {
    pub fn new() -> Self {
        Self {
            pieces: Vec::new(),
            caret: 0,
            anchor: None,
            pending: TextStyle::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    pub fn len(&self) -> usize {
        self.pieces.len()
    }

    pub fn is_empty(&self) -> bool {
        !self.pieces.iter().any(Piece::is_visible)
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    pub fn pending_style(&self) -> &TextStyle {
        &self.pending
    }

    pub fn grapheme_count(&self) -> usize {
        self.pieces
            .iter()
            .filter(|piece| matches!(piece, Piece::Grapheme { .. }))
            .count()
    }

    /// Half-open selection, caret order independent.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        let (start, end) = if anchor < self.caret {
            (anchor, self.caret)
        } else {
            (self.caret, anchor)
        };
        (start < end).then_some((start, end))
    }

    pub fn insert_text(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if text.is_empty() {
            return;
        }
        self.push_undo();
        self.delete_selection();
        let style = self.pending.clone();
        let graphemes: Vec<Piece> = text
            .graphemes(true)
            .map(|cluster| Piece::Grapheme {
                cluster: cluster.to_owned(),
                style: style.clone(),
            })
            .collect();
        let count = graphemes.len();
        self.pieces.splice(self.caret..self.caret, graphemes);
        self.caret += count;
        self.anchor = None;
    }

    pub fn insert_newline(&mut self) {
        self.insert_text("\n");
    }

    pub fn insert_piece(&mut self, piece: Piece) {
        self.push_undo();
        self.delete_selection();
        self.pieces.insert(self.caret, piece);
        self.caret += 1;
        self.anchor = None;
    }

    pub fn insert_wait(&mut self, ms: u32) {
        self.insert_piece(Piece::Wait { ms });
    }

    pub fn insert_speed(&mut self, ms_per_char: u32) {
        self.insert_piece(Piece::Speed {
            ms_per_char: ms_per_char.max(1),
        });
    }

    pub fn insert_instant(&mut self) {
        self.insert_piece(Piece::Instant);
    }

    pub fn insert_pause(&mut self) {
        self.insert_piece(Piece::Pause);
    }

    pub fn insert_image(&mut self, image: ImageAtom) {
        self.insert_piece(Piece::Image(image));
    }

    pub fn backspace(&mut self) {
        if self.selection_range().is_some() {
            self.push_undo();
            self.delete_selection();
            return;
        }
        if self.caret == 0 {
            return;
        }
        self.push_undo();
        self.caret -= 1;
        self.pieces.remove(self.caret);
    }

    pub fn delete_forward(&mut self) {
        if self.selection_range().is_some() {
            self.push_undo();
            self.delete_selection();
            return;
        }
        if self.caret >= self.pieces.len() {
            return;
        }
        self.push_undo();
        self.pieces.remove(self.caret);
    }

    pub fn move_caret(&mut self, delta: isize, selecting: bool) {
        self.place_caret(self.caret as isize + delta, selecting);
    }

    pub fn set_caret(&mut self, index: usize, selecting: bool) {
        self.place_caret(index as isize, selecting);
    }

    pub fn select_all(&mut self) {
        if self.pieces.is_empty() {
            self.anchor = None;
            self.caret = 0;
            return;
        }
        self.anchor = Some(0);
        self.caret = self.pieces.len();
    }

    /// Change the pending style. When a selection exists, graphemes inside it
    /// take the same edit. Markers are left alone.
    pub fn map_style(&mut self, f: impl Fn(&mut TextStyle)) {
        self.push_undo();
        if let Some((start, end)) = self.selection_range() {
            let mut seen = None;
            for piece in &mut self.pieces[start..end] {
                if let Piece::Grapheme { style, .. } = piece {
                    f(style);
                    seen = Some(style.clone());
                }
            }
            if let Some(style) = seen {
                self.pending = style;
            } else {
                f(&mut self.pending);
            }
        } else {
            f(&mut self.pending);
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.capture());
        self.restore(previous);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.capture());
        self.restore(next);
        true
    }

    /// Plain text only: markers and images are omitted. Newlines stay.
    pub fn plain_text(&self) -> String {
        self.plain_range(0, self.pieces.len())
    }

    pub fn selected_plain_text(&self) -> String {
        match self.selection_range() {
            Some((start, end)) => self.plain_range(start, end),
            None => self.plain_text(),
        }
    }

    /// Drop the sentence and its undo stack. Used after a successful submit.
    pub fn clear(&mut self) {
        self.pieces.clear();
        self.caret = 0;
        self.anchor = None;
        self.undo.clear();
        self.redo.clear();
    }

    /// Content copy for history. The undo stack is not part of a sentence.
    pub fn snapshot(&self) -> Self {
        Self {
            pieces: self.pieces.clone(),
            caret: self.pieces.len(),
            anchor: None,
            pending: self.pending.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Replace the draft with a recalled sentence. The previous draft stays on
    /// the undo stack so a mis-click can come back.
    pub fn replace_with(&mut self, other: DialogueDocument) {
        self.push_undo();
        self.pieces = other.pieces;
        self.caret = self.pieces.len();
        self.anchor = None;
        self.pending = other.pending;
    }

    fn plain_range(&self, start: usize, end: usize) -> String {
        let mut out = String::new();
        for piece in &self.pieces[start.min(self.pieces.len())..end.min(self.pieces.len())] {
            if let Piece::Grapheme { cluster, .. } = piece {
                out.push_str(cluster);
            }
        }
        out
    }

    fn place_caret(&mut self, index: isize, selecting: bool) {
        let index = index.clamp(0, self.pieces.len() as isize) as usize;
        if selecting {
            if self.anchor.is_none() {
                self.anchor = Some(self.caret);
            }
        } else {
            self.anchor = None;
        }
        self.caret = index;
        if self.anchor == Some(self.caret) {
            self.anchor = None;
        }
    }

    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection_range() {
            self.pieces.drain(start..end);
            self.caret = start;
            self.anchor = None;
        }
    }

    fn push_undo(&mut self) {
        self.undo.push(self.capture());
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn capture(&self) -> Snapshot {
        Snapshot {
            pieces: self.pieces.clone(),
            caret: self.caret,
            anchor: self.anchor,
            pending: self.pending.clone(),
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.pieces = snapshot.pieces;
        self.caret = snapshot.caret.min(self.pieces.len());
        self.anchor = snapshot.anchor;
        self.pending = snapshot.pending;
    }
}
