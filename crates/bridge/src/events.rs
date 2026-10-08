use serde::Serialize;

use crate::VtsHotkey;

/// Something worth telling other tools.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEvent {
    LineStart {
        id: u64,
        text: String,
        markup: String,
    },
    LineEnd {
        id: u64,
    },
    Expression {
        name: String,
        intensity: Option<f32>,
    },
    Portrait {
        variant: String,
    },
    Speaking {
        speaking: bool,
    },
    Shake {
        amp: f32,
        ms: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sink {
    Vts,
    Osc,
    WebSocket,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BridgeStatus {
    Off(Sink),
    Connecting(Sink),
    Connected(Sink),
    /// VTube Studio is reachable but the plugin is not authorized yet.
    VtsNeedsToken,
    /// A new token was granted; persist it.
    VtsToken(String),
    VtsHotkeys(Vec<VtsHotkey>),
    /// WebSocket clients currently connected.
    Clients(usize),
    Error(Sink, String),
}
