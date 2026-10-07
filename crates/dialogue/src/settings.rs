//! `%APPDATA%/NanaType/settings.json`. Geometry is remembered per monitor id.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::{DialogueChrome, INPUT_TOOLBAR_PX, ThemeId};
use crate::{AfterLine, DEFAULT_SPEED_MS};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    /// Physical-style name: `F10`, `Space`, `KeyN`, `Digit1`.
    pub key: String,
}

impl Hotkey {
    pub fn bare(key: impl Into<String>) -> Self {
        Self {
            ctrl: false,
            shift: false,
            alt: false,
            win: false,
            key: key.into(),
        }
    }

    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.win {
            parts.push("Win");
        }
        parts.push(self.key_label());
        parts.join("+")
    }

    fn key_label(&self) -> &str {
        self.key.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputPresence {
    /// The input window stays on the primary work area.
    Resident,
    /// Hidden until the summon hotkey. Escape or submit hides it again.
    Hotkey,
}

impl Default for InputPresence {
    fn default() -> Self {
        Self::Resident
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowFrame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: ThemeId,
    /// Global speaker. Empty hides the name plate everywhere.
    pub speaker_name: String,
    pub after_line: AfterLine,
    pub fade_base_ms: u32,
    pub fade_per_char_ms: u32,
    pub speed_ms: u32,
    pub simulate_typing: bool,
    pub simulate_pop: bool,
    /// Typewriter mirrors the draft and does not run reveal, wait, pause or fade.
    pub per_character: bool,
    pub typewriter_sound: bool,
    pub input_presence: InputPresence,
    pub summon_hotkey: Hotkey,
    pub continue_hotkey: Hotkey,
    pub layout_adjust: bool,
    pub show_input: bool,
    pub show_typewriter: bool,
    pub show_history: bool,
    pub spout_typewriter: bool,
    pub spout_history: bool,
    pub spout_name_typewriter: String,
    pub spout_name_history: String,
    pub input_width: f32,
    pub input_height: f32,
    pub typewriter_width: f32,
    pub typewriter_height: f32,
    pub history_width: f32,
    pub history_height: f32,
    pub sticker_dir: String,
    /// window key → monitor id → frame
    pub geometry: BTreeMap<String, BTreeMap<String, WindowFrame>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeId::Halo,
            speaker_name: String::new(),
            after_line: AfterLine::Hold,
            fade_base_ms: 1_200,
            fade_per_char_ms: 40,
            speed_ms: DEFAULT_SPEED_MS,
            simulate_typing: true,
            simulate_pop: true,
            per_character: false,
            typewriter_sound: false,
            input_presence: InputPresence::Resident,
            summon_hotkey: Hotkey::bare("F8"),
            continue_hotkey: Hotkey::bare("F10"),
            layout_adjust: false,
            show_input: true,
            show_typewriter: true,
            show_history: true,
            spout_typewriter: false,
            spout_history: false,
            spout_name_typewriter: "NanaType/Typewriter".into(),
            spout_name_history: "NanaType/History".into(),
            input_width: DialogueChrome::REGION_W,
            input_height: DialogueChrome::REGION_H + INPUT_TOOLBAR_PX,
            typewriter_width: DialogueChrome::REGION_W,
            typewriter_height: DialogueChrome::REGION_H,
            history_width: 420.0,
            history_height: 520.0,
            sticker_dir: String::new(),
            geometry: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("NanaType").join("settings.json")
    }

    /// Input stage and typewriter client share one dialogue rectangle from the
    /// theme. A saved pair that uses two different boxes is replaced by that
    /// rectangle; a pair that already matches is kept (layout adjust).
    pub fn align_dialogue_windows(&mut self) -> bool {
        let region_h = self.input_height - INPUT_TOOLBAR_PX;
        let matched = (self.input_width - self.typewriter_width).abs() < 1.0
            && (region_h - self.typewriter_height).abs() < 1.0
            && self.input_width >= 280.0
            && region_h >= 120.0;
        let (width, height) = if matched {
            (self.input_width, region_h)
        } else {
            (DialogueChrome::REGION_W, DialogueChrome::REGION_H)
        };
        let next = (width, height + INPUT_TOOLBAR_PX, width, height);
        let prev = (
            self.input_width,
            self.input_height,
            self.typewriter_width,
            self.typewriter_height,
        );
        self.input_width = next.0;
        self.input_height = next.1;
        self.typewriter_width = next.2;
        self.typewriter_height = next.3;
        prev != next
    }

    pub fn load() -> Self {
        let path = Self::path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        fs::write(path, text).map_err(|error| error.to_string())
    }

    pub fn frame(&self, window: &str, monitor: &str) -> Option<WindowFrame> {
        self.geometry.get(window)?.get(monitor).copied()
    }

    pub fn remember(&mut self, window: &str, monitor: &str, frame: WindowFrame) {
        self.geometry
            .entry(window.to_owned())
            .or_default()
            .insert(monitor.to_owned(), frame);
    }

    pub fn playback_config(&self) -> crate::PlaybackConfig {
        crate::PlaybackConfig {
            speed_ms: self.speed_ms.max(1),
            simulate_typing: self.simulate_typing && !self.per_character,
            simulate_pop: self.simulate_pop,
            after: self.after_line,
            fade_base_ms: self.fade_base_ms,
            fade_per_char_ms: self.fade_per_char_ms,
        }
    }
}
