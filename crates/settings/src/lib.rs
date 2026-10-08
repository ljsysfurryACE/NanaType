//! Persistent settings and window geometry.

#![forbid(unsafe_code)]

mod geometry;
mod migrate;
mod schema;
mod store;

#[cfg(test)]
mod tests;

pub use geometry::{
    GeometryStore, MonitorInfo, MonitorKey, Placement, SavedFrame, layout_signature, monitor_for,
};
pub use migrate::{MigrateError, migrate};
pub use schema::*;
pub use store::{LoadNote, Loaded, load_json, load_settings, save_json, write_atomic};
