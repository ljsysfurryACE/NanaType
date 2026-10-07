//! In-memory session history. Nothing here is written to disk.

use std::collections::VecDeque;

use crate::document::DialogueDocument;

/// Oldest entries fall off after this many submits.
pub const HISTORY_CAP: usize = 200;

#[derive(Clone)]
pub struct HistoryEntry {
    pub document: DialogueDocument,
    /// Speaker name copied at submit time. Later setting edits do not rewrite it.
    pub name: String,
}

#[derive(Clone, Default)]
pub struct SessionHistory {
    entries: VecDeque<HistoryEntry>,
}

impl SessionHistory {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &VecDeque<HistoryEntry> {
        &self.entries
    }

    pub fn get(&self, index: usize) -> Option<&HistoryEntry> {
        self.entries.get(index)
    }

    pub fn push(&mut self, document: DialogueDocument, name: impl Into<String>) {
        self.entries.push_back(HistoryEntry {
            document: document.snapshot(),
            name: name.into(),
        });
        while self.entries.len() > HISTORY_CAP {
            self.entries.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
