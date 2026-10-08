//! Character classes that pacing and line breaking care about.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PunctKind {
    /// `，、,;；：:` — a short breath.
    Comma,
    /// `。．.!！?？` — end of a clause.
    Stop,
    /// `…‥` and `——` — a trailing pause.
    Ellipsis,
    /// Brackets, quotes and other marks that do not pause.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharClass {
    Cjk,
    Latin,
    Punct(PunctKind),
    Space,
    /// Sticker, emote or image.
    Atom,
}

/// A grapheme is treated as CJK when any scalar is Han, kana, Hangul or a
/// full-width form. Full-width punctuation is classified first by [`classify`].
pub fn is_cjk_grapheme(cluster: &str) -> bool {
    cluster.chars().any(is_cjk_char)
}

fn is_cjk_char(c: char) -> bool {
    matches!(
        c,
        '\u{1100}'..='\u{11FF}'
            | '\u{2E80}'..='\u{2FFF}'
            | '\u{3040}'..='\u{9FFF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF66}'..='\u{FF9D}'
            | '\u{20000}'..='\u{3134F}'
    )
}

fn punct_kind(c: char) -> Option<PunctKind> {
    Some(match c {
        ',' | '，' | '、' | ';' | '；' | ':' | '：' | '､' => PunctKind::Comma,
        '.' | '。' | '．' | '!' | '！' | '?' | '？' | '｡' => PunctKind::Stop,
        '…' | '‥' | '—' | '―' | '～' | '~' => PunctKind::Ellipsis,
        c if c.is_ascii_punctuation() => PunctKind::Other,
        '「' | '」' | '『' | '』' | '（' | '）' | '【' | '】' | '《' | '》' | '〈' | '〉' | '“'
        | '”' | '‘' | '’' | '·' | '・' | '〜' | '〔' | '〕' => PunctKind::Other,
        _ => return None,
    })
}

/// Classify one grapheme cluster.
pub fn classify(cluster: &str) -> CharClass {
    let mut chars = cluster.chars();
    let Some(first) = chars.next() else {
        return CharClass::Space;
    };
    if cluster.chars().all(char::is_whitespace) {
        return CharClass::Space;
    }
    if let Some(kind) = punct_kind(first) {
        return CharClass::Punct(kind);
    }
    if is_cjk_grapheme(cluster) {
        CharClass::Cjk
    } else {
        CharClass::Latin
    }
}
