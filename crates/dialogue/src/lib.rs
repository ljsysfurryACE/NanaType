//! Dialogue document, playback clock, theme packs and settings.
//!
//! This crate does not know about windows, GPU or Spout. The host paints
//! whatever [`DialogueDocument`] and [`Playback`] describe.

mod document;
mod history;
mod playback;
mod settings;
mod theme;

#[cfg(test)]
mod tests;

pub use document::{
    DialogueDocument, ImageAtom, ImageData, ImageFrame, ImageKind, Piece, TextStyle,
};
pub use history::{HISTORY_CAP, HistoryEntry, SessionHistory};
pub use playback::{AfterLine, Phase, Playback, PlaybackConfig, pop_progress, pop_scale};
pub use settings::{Hotkey, InputPresence, Settings, WindowFrame};
pub use theme::{DialogueChrome, INPUT_TOOLBAR_PX, Motion, ThemeId, ThemePack, UiColor, UiPalette};

/// A grapheme is treated as CJK when any scalar is Han, Hiragana, Katakana or Hangul.
pub fn is_cjk_grapheme(cluster: &str) -> bool {
    cluster.chars().any(is_cjk_char)
}

fn is_cjk_char(c: char) -> bool {
    matches!(
        c,
        '\u{1100}'..='\u{11FF}'
            | '\u{2E80}'..='\u{9FFF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF66}'..='\u{FF9D}'
    )
}

/// Default gap between revealed graphemes, in milliseconds.
pub const DEFAULT_SPEED_MS: u32 = 50;
/// How long a pop-in takes.
pub const POP_MS: u64 = 180;
