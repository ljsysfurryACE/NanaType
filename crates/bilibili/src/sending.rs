use crate::ApiError;
use nanatype_model::{Document, Inline, LineId};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryState {
    Queued,
    Sending,
    Submitted,
    Failed,
    Uncertain,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    pub line: LineId,
    pub room: u64,
    pub uid: u64,
    pub chunks: Vec<String>,
    pub completed: usize,
    pub state: DeliveryState,
    pub detail: String,
    /// This delivery owns the paused queue head.
    pub blocked: bool,
    pub receipts: Vec<String>,
}
impl Delivery {
    pub fn remaining(&self) -> String {
        self.chunks[self.completed..].join("")
    }
    pub fn dedup_confirmed(&self) -> bool {
        self.completed == self.receipts.len()
    }
}
pub fn image_count(doc: &Document) -> usize {
    doc.paragraphs
        .iter()
        .flat_map(|p| &p.inlines)
        .filter(|i| matches!(i, Inline::Atom(_)))
        .count()
}
pub fn plain_text(doc: &Document) -> Result<String, ApiError> {
    if image_count(doc) > 0 {
        return Err(ApiError::InvalidInput(
            "images unavailable during danmaku synchronization".into(),
        ));
    }
    let text = doc
        .plain_text()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return Err(ApiError::InvalidInput("no readable text".into()));
    }
    Ok(text)
}
/// Count Unicode scalars while preserving extended grapheme clusters.
pub fn split(text: &str, limit: usize) -> Result<Vec<String>, ApiError> {
    if limit == 0 {
        return Err(ApiError::InvalidInput("invalid danmaku length".into()));
    }
    let gs: Vec<&str> = text.graphemes(true).collect();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < gs.len() {
        let mut end = start;
        let mut count = 0;
        let mut punctuation = None;
        while end < gs.len() && count + gs[end].chars().count() <= limit {
            count += gs[end].chars().count();
            end += 1;
            if gs[end - 1].chars().any(|c| "。！？；，,.!?; ".contains(c)) {
                punctuation = Some(end);
            }
        }
        if end == start {
            return Err(ApiError::InvalidInput(
                "one combined character exceeds the danmaku limit".into(),
            ));
        }
        if end < gs.len()
            && let Some(p) = punctuation.filter(|p| *p > start)
        {
            end = p;
        }
        let chunk = gs[start..end].concat();
        if !chunk.trim().is_empty() {
            chunks.push(chunk);
        }
        start = end;
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunks_preserve_content_and_combined_characters() {
        for text in [
            "你好，世界。这是台词",
            "hello world! again",
            "a🇹🇼e\u{301}👨‍👩‍👧‍👦x",
        ] {
            let chunks = split(text, 12).unwrap();
            assert_eq!(chunks.concat(), text);
            assert!(chunks.iter().all(|s| s.chars().count() <= 12));
        }
        assert!(split("👨‍👩‍👧‍👦", 2).is_err());
    }
    #[test]
    fn transmission_removes_performance_not_text() {
        let doc = Document::plain("你好\n 世界");
        assert_eq!(plain_text(&doc).unwrap(), "你好 世界");
        use nanatype_model::{
            AssetRef, Atom, AtomKind, ChatUser, Marker, Mention, RubyGroup, TextRun,
        };
        let mut text = TextRun::plain("你好 ");
        text.style.bold = Some(true);
        let mut doc = Document::from_inlines(vec![
            Inline::Text(text),
            Inline::Ruby(RubyGroup {
                base: TextRun::plain("世界"),
                annotation: "せかい".into(),
            }),
            Inline::Marker(Marker::Wait { ms: 1000 }),
            Inline::Mention(Mention::new(ChatUser {
                uid: "7".into(),
                name: "观众".into(),
                avatar: None,
            })),
        ]);
        assert_eq!(plain_text(&doc).unwrap(), "你好 世界@观众");
        doc.paragraphs[0].inlines.push(Inline::Atom(Atom {
            kind: AtomKind::Image,
            asset: AssetRef::Blob("fixture".into()),
            height_em: 1.0,
            alt: "picture".into(),
            effects: Default::default(),
        }));
        assert!(plain_text(&doc).is_err());
    }
}
