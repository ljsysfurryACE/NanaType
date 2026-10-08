//! Window placement that survives reboots, docking and monitor changes.
//!
//! Positions are stored per *monitor layout* (the set of connected monitors)
//! and relative to a monitor's work area, keyed by a stable monitor identity
//! (device path on Windows, display UUID on macOS) instead of a handle.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MonitorKey(pub String);

/// A monitor as the host sees it now.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    pub key: MonitorKey,
    /// Work area in logical (desktop) coordinates: `[x, y, w, h]`.
    pub work_area: [f64; 4],
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedFrame {
    pub monitor: MonitorKey,
    /// Top-left corner as a fraction of the work area (0..1).
    pub rel: [f64; 2],
    pub size: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GeometryStore {
    /// Layout signature → window key → frame.
    pub profiles: BTreeMap<String, BTreeMap<String, SavedFrame>>,
}

/// Placement result in logical desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn layout_signature(monitors: &[MonitorInfo]) -> String {
    let mut keys: Vec<&str> = monitors.iter().map(|m| m.key.0.as_str()).collect();
    keys.sort_unstable();
    keys.join("|")
}

fn to_placement(frame: &SavedFrame, monitor: &MonitorInfo) -> Placement {
    let [x, y, w, h] = monitor.work_area;
    let size_w = frame.size[0].min(w.max(1.0));
    let size_h = frame.size[1].min(h.max(1.0));
    let px = (x + frame.rel[0] * w).clamp(x, x + (w - size_w).max(0.0));
    let py = (y + frame.rel[1] * h).clamp(y, y + (h - size_h).max(0.0));
    Placement {
        x: px,
        y: py,
        w: size_w,
        h: size_h,
    }
}

impl GeometryStore {
    pub fn remember(&mut self, monitors: &[MonitorInfo], window: &str, rect: Placement) {
        let Some(monitor) = monitor_for(monitors, rect) else {
            return;
        };
        let [x, y, w, h] = monitor.work_area;
        let frame = SavedFrame {
            monitor: monitor.key.clone(),
            rel: [
                ((rect.x - x) / w.max(1.0)).clamp(0.0, 1.0),
                ((rect.y - y) / h.max(1.0)).clamp(0.0, 1.0),
            ],
            size: [rect.w, rect.h],
        };
        self.profiles
            .entry(layout_signature(monitors))
            .or_default()
            .insert(window.to_owned(), frame);
    }

    /// Exact layout first, then the newest frame on a monitor that is still
    /// connected. `None` means "use the default placement".
    pub fn restore(&self, monitors: &[MonitorInfo], window: &str) -> Option<Placement> {
        let find = |frame: &SavedFrame| {
            monitors
                .iter()
                .find(|m| m.key == frame.monitor)
                .map(|m| to_placement(frame, m))
        };
        if let Some(frame) = self
            .profiles
            .get(&layout_signature(monitors))
            .and_then(|p| p.get(window))
            && let Some(p) = find(frame)
        {
            return Some(p);
        }
        self.profiles
            .values()
            .filter_map(|p| p.get(window))
            .find_map(find)
    }

    pub fn forget(&mut self, window: &str) {
        for profile in self.profiles.values_mut() {
            profile.remove(window);
        }
    }
}

/// The monitor holding the rectangle's centre, else the primary one.
pub fn monitor_for(monitors: &[MonitorInfo], rect: Placement) -> Option<&MonitorInfo> {
    let cx = rect.x + rect.w / 2.0;
    let cy = rect.y + rect.h / 2.0;
    monitors
        .iter()
        .find(|m| {
            let [x, y, w, h] = m.work_area;
            cx >= x && cx < x + w && cy >= y && cy < y + h
        })
        .or_else(|| monitors.iter().find(|m| m.primary))
        .or_else(|| monitors.first())
}
