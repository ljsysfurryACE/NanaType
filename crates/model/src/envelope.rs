//! Versioned JSON wrapper for documents that leave the process (clipboard,
//! quick phrases, the WebSocket broadcast).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Document;

pub const DOCUMENT_FORMAT: &str = "nanatype.document";
pub const DOCUMENT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentEnvelope {
    pub format: String,
    pub v: u32,
    pub doc: Document,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    Json(String),
    WrongFormat(String),
    /// Written by a newer NanaType than this one.
    TooNew(u32),
}

impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "malformed document: {error}"),
            Self::WrongFormat(format) => write!(f, "not a NanaType document: {format}"),
            Self::TooNew(v) => write!(f, "document version {v} is newer than this app"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

impl DocumentEnvelope {
    pub fn new(doc: Document) -> Self {
        Self {
            format: DOCUMENT_FORMAT.to_owned(),
            v: DOCUMENT_VERSION,
            doc,
        }
    }

    pub fn to_json(doc: &Document) -> String {
        serde_json::to_string(&Self::new(doc.clone())).expect("documents always serialize")
    }

    /// Parse, migrating older versions step by step.
    pub fn from_json(text: &str) -> Result<Document, EnvelopeError> {
        let mut value: Value =
            serde_json::from_str(text).map_err(|e| EnvelopeError::Json(e.to_string()))?;
        let format = value
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if format != DOCUMENT_FORMAT {
            return Err(EnvelopeError::WrongFormat(format));
        }
        let v = value.get("v").and_then(Value::as_u64).unwrap_or(1) as u32;
        if v > DOCUMENT_VERSION {
            return Err(EnvelopeError::TooNew(v));
        }
        // Version 1 is the first persisted shape; later migrations chain here.
        let doc = value
            .get_mut("doc")
            .map(Value::take)
            .ok_or_else(|| EnvelopeError::Json("missing `doc`".into()))?;
        let mut doc: Document =
            serde_json::from_value(doc).map_err(|e| EnvelopeError::Json(e.to_string()))?;
        doc.normalize();
        Ok(doc)
    }
}
