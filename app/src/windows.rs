//! The four windows and how settings map onto them.
//!
//! Each frame of state changes computes the *desired* native state of every
//! window from the settings and the skin, then [`WindowManager::reconcile`]
//! issues only the commands that differ from what was last applied.

use std::collections::HashMap;

use nana_ui::{MaterialEffect, WindowDescriptor, WindowHandle};
use nana_ui_platform::host::WindowCommand;
use nana_ui_platform::{WindowId, WindowRole};
use nanatype_settings::{
    GeometryStore, InputPresence, Lock, MonitorInfo, OverlaySettings, Placement, Settings,
};
use nanatype_theme::Skin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowKind {
    Input,
    Display,
    History,
    Settings,
}

/// Height of the input window's toolbar.
pub const TOOLBAR_H: f32 = 48.0;
/// Status line at the bottom of the input window.
pub const STATUS_H: f32 = 26.0;

impl WindowKind {
    pub const OVERLAYS: [Self; 3] = [Self::Input, Self::Display, Self::History];

    pub fn id(self) -> WindowId {
        match self {
            Self::Input => WindowId::PRIMARY,
            Self::Display => WindowId(2),
            Self::History => WindowId(3),
            Self::Settings => WindowId(4),
        }
    }

    pub fn from_id(id: WindowId) -> Option<Self> {
        [Self::Input, Self::Display, Self::History, Self::Settings]
            .into_iter()
            .find(|k| k.id() == id)
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Input => "nt.input",
            Self::Display => "nt.display",
            Self::History => "nt.history",
            Self::Settings => "nt.settings",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Display => "display",
            Self::History => "history",
            Self::Settings => "settings",
        }
    }

    pub fn title_key(self) -> &'static str {
        match self {
            Self::Input => "window-input",
            Self::Display => "window-display",
            Self::History => "window-history",
            Self::Settings => "window-settings",
        }
    }

    pub fn overlay(self, s: &Settings) -> Option<&OverlaySettings> {
        match self {
            Self::Input => Some(&s.overlays.input),
            Self::Display => Some(&s.overlays.display),
            Self::History => Some(&s.overlays.history),
            Self::Settings => None,
        }
    }

    pub fn overlay_mut(self, s: &mut Settings) -> Option<&mut OverlaySettings> {
        match self {
            Self::Input => Some(&mut s.overlays.input),
            Self::Display => Some(&mut s.overlays.display),
            Self::History => Some(&mut s.overlays.history),
            Self::Settings => None,
        }
    }

    pub fn material(self) -> MaterialEffect {
        match self {
            Self::Settings => MaterialEffect::Solid,
            _ if debug_opaque() => MaterialEffect::Solid,
            _ => MaterialEffect::Transparent,
        }
    }
}

/// Development aid: `NANATYPE_OPAQUE=1` paints overlays on a solid
/// background so a screen grab of their rectangle shows only NanaType.
pub fn debug_opaque() -> bool {
    cfg!(debug_assertions) && std::env::var_os("NANATYPE_OPAQUE").is_some()
}

/// Logical size of a window for the current skin and settings.
pub fn size_of(kind: WindowKind, skin: &Skin, s: &Settings) -> (f32, f32) {
    let d = &skin.dialogue;
    let region = s.skin.overrides.region.unwrap_or(d.region);
    let pad = d.overscan * 2.0;
    match kind {
        WindowKind::Display => s
            .overlays
            .display
            .size
            .map(|[w, h]| (w, h))
            .unwrap_or((region[0] + pad, region[1] + pad)),
        WindowKind::Input => (region[0] + pad, TOOLBAR_H + region[1] + pad + STATUS_H),
        WindowKind::History => s
            .overlays
            .history
            .size
            .map(|[w, h]| (w, h))
            .unwrap_or((skin.history.width, skin.history.height)),
        WindowKind::Settings => (920.0, 660.0),
    }
}

/// The app icon for window title bars and the taskbar.
pub fn icon() -> Option<nana_ui_platform::WindowIcon> {
    static ICON: std::sync::OnceLock<Option<(Vec<u8>, u32, u32)>> = std::sync::OnceLock::new();
    let (rgba, w, h) = ICON
        .get_or_init(|| {
            let img = image::load_from_memory(include_bytes!("../assets/tray-64.png"))
                .ok()?
                .into_rgba8();
            let (w, h) = img.dimensions();
            Some((img.into_raw(), w, h))
        })
        .clone()?;
    nana_ui_platform::WindowIcon::from_rgba(rgba, w, h).ok()
}

pub fn descriptor(kind: WindowKind, title: &str, skin: &Skin, s: &Settings) -> WindowDescriptor {
    let (w, h) = size_of(kind, skin, s);
    let mut d = WindowDescriptor::new(title).initial_size(f64::from(w), f64::from(h));
    d.tag = Some(kind.tag().into());
    d.icon = icon();
    match kind {
        WindowKind::Settings => {
            d.minimum_size = (720.0, 480.0);
            d.system_caption = true;
            d.resizable = true;
            d.visible = true;
            d.focus_on_show = true;
            d.role = WindowRole::Main;
        }
        _ => {
            d.transparent = !debug_opaque();
            d.shadow = nana_ui::WindowShadow::None;
            d.always_on_top = kind.overlay(s).is_some_and(|o| o.always_on_top);
            d.skip_taskbar = true;
            d.role = WindowRole::Tool;
            d.focus_on_show = kind == WindowKind::Input;
            d.constrain_to_work_area = true;
            d.resizable = kind == WindowKind::History;
            d.system_caption = false;
            // Shown once placed, so nothing flashes at the default origin.
            d.visible = false;
        }
    }
    d
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Off,
    Full,
    Forward,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Desired {
    pub visible: bool,
    pub pass: Pass,
    pub on_top: bool,
    pub resizable: bool,
    pub size: (f32, f32),
}

pub fn desired(kind: WindowKind, skin: &Skin, s: &Settings, input_summoned: bool) -> Desired {
    let o = kind.overlay(s);
    let visible = match kind {
        WindowKind::Input => {
            s.overlays.input.visible
                && (s.general.input_presence == InputPresence::Resident || input_summoned)
        }
        WindowKind::Settings => true,
        _ => o.is_some_and(|o| o.visible),
    };
    let lock = o.map(|o| o.lock).unwrap_or(Lock::Interactive);
    let pass = match (kind, lock) {
        (WindowKind::Input | WindowKind::Settings, _) => Pass::Off,
        (_, Lock::Locked) => Pass::Full,
        (_, Lock::Interactive) => Pass::Forward,
        (_, Lock::EditLayout) => Pass::Off,
    };
    Desired {
        visible,
        pass,
        on_top: o.is_some_and(|o| o.always_on_top),
        resizable: matches!(kind, WindowKind::History | WindowKind::Display)
            && lock == Lock::EditLayout,
        size: size_of(kind, skin, s),
    }
}

#[derive(Default)]
struct Slot {
    handle: Option<WindowHandle>,
    applied: Option<Desired>,
    /// Logical top-left and size last reported by the window.
    frame: Option<Placement>,
    placed: bool,
}

#[derive(Default)]
pub struct WindowManager {
    slots: HashMap<WindowKind, Slot>,
    pub monitors: Vec<MonitorInfo>,
    pub geometry: GeometryStore,
    pub geometry_dirty: bool,
}

impl WindowManager {
    pub fn is_open(&self, kind: WindowKind) -> bool {
        self.slots.get(&kind).is_some_and(|s| s.handle.is_some())
    }

    pub fn handle(&self, kind: WindowKind) -> Option<&WindowHandle> {
        self.slots.get(&kind).and_then(|s| s.handle.as_ref())
    }

    pub fn ready(&mut self, kind: WindowKind, handle: WindowHandle) {
        let slot = self.slots.entry(kind).or_default();
        slot.handle = Some(handle);
        slot.applied = None;
        slot.placed = false;
    }

    pub fn closed(&mut self, kind: WindowKind) {
        self.slots.remove(&kind);
    }

    pub fn frame(&self, kind: WindowKind) -> Option<Placement> {
        self.slots.get(&kind).and_then(|s| s.frame)
    }

    /// The window moved or resized (logical coordinates).
    pub fn moved(&mut self, kind: WindowKind, position: Option<(f32, f32)>, size: (f32, f32)) {
        let slot = self.slots.entry(kind).or_default();
        let prev = slot.frame;
        let (x, y) = position
            .map(|(x, y)| (f64::from(x), f64::from(y)))
            .or_else(|| prev.map(|p| (p.x, p.y)))
            .unwrap_or((0.0, 0.0));
        let frame = Placement {
            x,
            y,
            w: f64::from(size.0),
            h: f64::from(size.1),
        };
        slot.frame = Some(frame);
        if slot.placed
            && prev != Some(frame)
            && kind != WindowKind::Settings
            && !self.monitors.is_empty()
        {
            self.geometry.remember(&self.monitors, kind.key(), frame);
            self.geometry_dirty = true;
        }
    }

    /// Default spot: input bottom-centre of the primary work area, display
    /// above it, history at the right edge.
    fn default_place(&self, kind: WindowKind, size: (f32, f32)) -> Option<(f32, f32)> {
        let m = self
            .monitors
            .iter()
            .find(|m| m.primary)
            .or_else(|| self.monitors.first())?;
        let [x, y, w, h] = m.work_area;
        let (sw, sh) = (f64::from(size.0), f64::from(size.1));
        let margin = 24.0;
        let (px, py) = match kind {
            WindowKind::Input => (x + (w - sw) / 2.0, y + h - sh - margin),
            WindowKind::Display => (x + (w - sw) / 2.0, y + h * 0.55 - sh / 2.0),
            WindowKind::History => (x + w - sw - margin, y + (h - sh) / 2.0),
            WindowKind::Settings => (x + (w - sw) / 2.0, y + (h - sh) / 2.0),
        };
        Some((px.max(x) as f32, py.max(y) as f32))
    }

    pub fn forget(&mut self, kind: WindowKind) {
        self.geometry.forget(kind.key());
        self.geometry_dirty = true;
        if let Some(slot) = self.slots.get_mut(&kind) {
            slot.placed = false;
            slot.applied = None;
        }
    }

    /// Commands that bring every open window to its desired state.
    pub fn reconcile(&mut self, want: &[(WindowKind, Desired)]) -> Vec<WindowCommand> {
        let mut out = Vec::new();
        let monitors_known = !self.monitors.is_empty();
        for (kind, d) in want {
            let placement = self.geometry.restore(&self.monitors, kind.key());
            let default = self.default_place(*kind, d.size);
            let Some(slot) = self.slots.get_mut(kind) else {
                continue;
            };
            let Some(handle) = slot.handle.clone() else {
                continue;
            };
            let id = kind.id();
            let prev = slot.applied.clone();
            if !slot.placed && monitors_known {
                let (pos, size) = match (*kind, placement) {
                    // Overlays whose size follows the skin keep that size.
                    (WindowKind::History, Some(p)) => {
                        ((p.x as f32, p.y as f32), (p.w as f32, p.h as f32))
                    }
                    (_, Some(p)) => ((p.x as f32, p.y as f32), d.size),
                    (_, None) => (default.unwrap_or((100.0, 100.0)), d.size),
                };
                out.push(WindowCommand::SetBounds {
                    id,
                    position: pos,
                    size,
                });
                slot.placed = true;
            } else if prev.as_ref().is_some_and(|p| p.size != d.size)
                && !(*kind == WindowKind::History && prev.as_ref().is_some_and(|p| p.resizable))
                && let Some(f) = slot.frame
            {
                out.push(WindowCommand::SetBounds {
                    id,
                    position: (f.x as f32, f.y as f32),
                    size: d.size,
                });
            }
            if prev.as_ref().map(|p| p.pass) != Some(d.pass) && *kind != WindowKind::Settings {
                out.push(WindowCommand::SetMousePassthrough {
                    id,
                    enabled: d.pass == Pass::Full,
                });
                out.push(WindowCommand::SetMousePassthroughForward {
                    id,
                    enabled: d.pass == Pass::Forward,
                });
            }
            if prev.as_ref().map(|p| p.on_top) != Some(d.on_top) && *kind != WindowKind::Settings {
                out.push(WindowCommand::SetAlwaysOnTop {
                    id,
                    always_on_top: d.on_top,
                });
            }
            if prev.as_ref().map(|p| p.resizable) != Some(d.resizable) {
                drop(handle.set_resizable(d.resizable));
            }
            let visible = d.visible && (slot.placed || !monitors_known);
            if prev.as_ref().map(|p| p.visible) != Some(visible) {
                crate::debug_log(&format!(
                    "{kind:?} set_visible({visible}) placed={} monitors={monitors_known}",
                    slot.placed
                ));
                drop(handle.set_visible(visible));
            }
            slot.applied = Some(Desired {
                visible,
                ..d.clone()
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin() -> Skin {
        nanatype_theme::SkinRegistry::new(None, "1.0.0")
            .load("nanatype.halo")
            .unwrap()
            .skin
            .as_ref()
            .clone()
    }

    #[test]
    fn display_and_input_share_the_dialogue_width() {
        let skin = skin();
        let s = Settings::default();
        let (dw, dh) = size_of(WindowKind::Display, &skin, &s);
        let (iw, ih) = size_of(WindowKind::Input, &skin, &s);
        assert_eq!(dw, iw);
        assert_eq!(dw, skin.dialogue.region[0] + skin.dialogue.overscan * 2.0);
        assert!(ih > dh, "the input adds a toolbar and an editor");
    }

    #[test]
    fn lock_modes_map_to_passthrough() {
        let skin = skin();
        let mut s = Settings::default();
        s.overlays.display.lock = Lock::Locked;
        assert_eq!(
            desired(WindowKind::Display, &skin, &s, false).pass,
            Pass::Full
        );
        s.overlays.display.lock = Lock::Interactive;
        assert_eq!(
            desired(WindowKind::Display, &skin, &s, false).pass,
            Pass::Forward
        );
        s.overlays.display.lock = Lock::EditLayout;
        let d = desired(WindowKind::Display, &skin, &s, false);
        assert_eq!(d.pass, Pass::Off);
        assert!(d.resizable);
        assert_eq!(desired(WindowKind::Input, &skin, &s, false).pass, Pass::Off);
    }

    #[test]
    fn summon_presence_hides_the_input_until_summoned() {
        let skin = skin();
        let mut s = Settings::default();
        s.general.input_presence = InputPresence::Summon;
        assert!(!desired(WindowKind::Input, &skin, &s, false).visible);
        assert!(desired(WindowKind::Input, &skin, &s, true).visible);
    }

    #[test]
    fn a_size_override_wins_over_the_skin() {
        let skin = skin();
        let mut s = Settings::default();
        s.overlays.history.size = Some([300.0, 900.0]);
        assert_eq!(size_of(WindowKind::History, &skin, &s), (300.0, 900.0));
    }
}
