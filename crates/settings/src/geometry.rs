//! Window placement that survives reboots, docking and monitor changes.
//!
//! Positions are stored per *monitor layout* (the set of connected monitors)
//! and relative to a monitor's work area, keyed by a stable monitor identity
//! (device path on Windows, display UUID on macOS) instead of a handle.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const INPUT_SIDEBAR_DEFAULT: f32 = 300.0;
pub const INPUT_SIDEBAR_MIN: f32 = 160.0;
pub const INPUT_SIDEBAR_MAX: f32 = 560.0;

pub fn input_sidebar_width(value: Option<f32>) -> f32 {
    value
        .filter(|v| v.is_finite())
        .unwrap_or(INPUT_SIDEBAR_DEFAULT)
        .clamp(INPUT_SIDEBAR_MIN, INPUT_SIDEBAR_MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDockRegionKind {
    Commands,
    Stickers,
    Phrases,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputDockRegion {
    pub id: u64,
    pub kind: InputDockRegionKind,
    pub number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputDockLayout {
    pub regions: Vec<InputDockRegion>,
    pub selected: u64,
}

impl Default for InputDockLayout {
    fn default() -> Self {
        Self {
            regions: vec![
                InputDockRegion {
                    id: 0,
                    kind: InputDockRegionKind::Commands,
                    number: 1,
                },
                InputDockRegion {
                    id: 1,
                    kind: InputDockRegionKind::Stickers,
                    number: 1,
                },
            ],
            selected: 0,
        }
    }
}

impl InputDockLayout {
    /// Keep saved order and identities; repair stale or unsupported entries.
    pub fn normalized(mut self) -> Self {
        let mut ids = std::collections::HashSet::new();
        self.regions
            .retain(|r| r.kind != InputDockRegionKind::Unknown && ids.insert(r.id));
        if self.regions.is_empty() {
            return Self::default();
        }
        for region in &mut self.regions {
            region.number = region.number.max(1);
        }
        if !self.regions.iter().any(|r| r.id == self.selected) {
            self.selected = self.regions[0].id;
        }
        self
    }
}

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
    /// Preferred logical width, independent of temporary small-window limits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_sidebar_width: Option<f32>,
    /// Input workspace regions and active tab, shared across monitor layouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_dock: Option<InputDockLayout>,
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
