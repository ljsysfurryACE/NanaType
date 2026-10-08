use std::collections::VecDeque;

use nanatype_model::Line;
use serde::{Deserialize, Serialize};

/// What sending a line does while another is still on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePolicy {
    /// The new line replaces the current one immediately.
    #[default]
    Replace,
    /// The current line finishes its reveal at once, then the new one starts.
    FinishThenNext,
    /// Wait until the current line is done (and its auto-advance or the
    /// continue key), then start the new one.
    Queue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueueDecision {
    StartNow(Line),
    /// Fast-forward the current line, then start this one.
    FinishCurrentThenStart(Line),
    Enqueued {
        position: usize,
    },
}

#[derive(Debug, Default)]
pub struct LineQueue {
    pending: VecDeque<Line>,
    pub policy: QueuePolicy,
}

impl LineQueue {
    pub fn new(policy: QueuePolicy) -> Self {
        Self {
            pending: VecDeque::new(),
            policy,
        }
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// `busy` is whether a line is still revealing or gated.
    pub fn submit(&mut self, line: Line, busy: bool) -> QueueDecision {
        if !busy && self.pending.is_empty() {
            return QueueDecision::StartNow(line);
        }
        match self.policy {
            QueuePolicy::Replace => {
                self.pending.clear();
                QueueDecision::StartNow(line)
            }
            QueuePolicy::FinishThenNext => {
                self.pending.clear();
                QueueDecision::FinishCurrentThenStart(line)
            }
            QueuePolicy::Queue => {
                self.pending.push_back(line);
                QueueDecision::Enqueued {
                    position: self.pending.len(),
                }
            }
        }
    }

    /// The next line, when the current one is done and allowed to move on.
    pub fn pop_next(&mut self) -> Option<Line> {
        self.pending.pop_front()
    }
}
