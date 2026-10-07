//! NanaType host: one hidden root, the input, the typewriter, history and settings.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use dialogue::{
    AfterLine, DialogueDocument, HistoryEntry, ImageKind, InputPresence, Motion, Piece, Playback,
    SessionHistory, ThemePack,
};
use nana_ui::runtime::{
    DocumentId, Entity, FrameworkError, RuntimeDocument, TextInput, TextInputState,
};
use nana_ui::{
    CompositionInput, DocumentAccessError, FileDialogKind, FileDialogRequest, FileFilter,
    FrameDemand, GpuBackendPolicy, InputPayload, KeyState, MaterialEffect, PointerPhase,
    RoutedInput, RuntimeProgram, RuntimeProgramContext, RuntimeProgramUpdate, WheelUnit,
    WindowDescriptor, WindowHandle, WindowShadow, WindowSurfacePreference,
};
use nana_ui_platform::host::WindowCommand;
use nana_ui_platform::{WindowEvent, WindowGeometry, WindowId, WindowRole};

use crate::geom;
use crate::hotkeys::{HotkeyHit, Hotkeys};
use crate::images;
use crate::paint::{self, Frame, Painter, Scene};
use crate::sound;
use crate::surface::{self, Lane};
use crate::theme_ui;
use crate::tray::{self, Tray, TrayAction};
use crate::ui::{self, InputUi, SettingsUi};

const INPUT: WindowId = WindowId(1);
const TYPEWRITER: WindowId = WindowId(2);
const HISTORY: WindowId = WindowId(3);
const SETTINGS: WindowId = WindowId(4);
const DIALOG_IMAGE: u64 = 1;
const DIALOG_STICKER: u64 = 2;

#[derive(Clone, Debug)]
pub enum Message {
    Bold,
    Italic,
    Underline,
    ColorAccent,
    ColorBody,
    Larger,
    Smaller,
    Motion(Motion),
    Wait,
    SpeedMark,
    Instant,
    Pause,
    PickImage,
    PickStickers,
    NextSticker,
    Paste,
    Submit,
    Theme(dialogue::ThemeId),
    ToggleTyping,
    TogglePop,
    TogglePerChar,
    ToggleSound,
    ToggleFade,
    PresenceResident,
    PresenceHotkey,
    LayoutAdjust,
    CaptureSummon,
    CaptureContinue,
    ShowInput,
    ShowTypewriter,
    ShowHistory,
    SpoutTypewriter,
    SpoutHistory,
}

enum Capture {
    Summon,
    Continue,
}

struct Hosted {
    document: RuntimeDocument,
}

pub struct App {
    settings: dialogue::Settings,
    theme: Arc<nana_ui::CompiledTheme>,
    started: Instant,
    draft: DialogueDocument,
    playback: Option<Playback>,
    history: SessionHistory,
    history_pos: Option<usize>,
    pop_started: Option<u64>,
    painter: Painter,
    input_frame: Option<Frame>,
    history_frame: Option<Frame>,
    input_lane: Lane,
    typewriter_lane: Lane,
    history_lane: Lane,
    windows: HashMap<WindowId, Hosted>,
    handles: HashMap<WindowId, WindowHandle>,
    input_ui: Option<InputUi>,
    settings_ui: Option<SettingsUi>,
    children_open: bool,
    settings_open: bool,
    tray: Tray,
    hotkeys: Hotkeys,
    capture: Option<Capture>,
    suppress: Option<String>,
    preedit: String,
    selecting: bool,
    sticker_index: usize,
    last_continue: Instant,
    dirty: bool,
    last_save: Instant,
    status: String,
    next_wake: Instant,
    input_rev: u64,
    input_painted: u64,
    history_rev: u64,
    history_painted: u64,
    typewriter_rev: u64,
    typewriter_painted: u64,
    history_height_l: f32,
    history_scale: f32,
    history_scroll: f32,
    history_pinned: bool,
    /// Dialogue rectangle shared by the input stage and the typewriter client.
    region: (f32, f32),
    paint_clock: PaintClock,
}

impl App {
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn pack(&self) -> ThemePack {
        ThemePack::get(self.settings.theme)
    }

    fn open_children(&mut self) -> RuntimeProgramUpdate {
        if self.children_open {
            return RuntimeProgramUpdate::default();
        }
        self.children_open = true;
        let mut commands = Vec::new();
        if let Ok(command) = self.prepare_window(INPUT, "输入", true) {
            commands.push(command);
        }
        if let Ok(command) = self.prepare_window(TYPEWRITER, "打字机", false) {
            commands.push(command);
        }
        if let Ok(command) = self.prepare_window(HISTORY, "历史", false) {
            commands.push(command);
        }
        RuntimeProgramUpdate {
            redraw: nana_ui::RuntimeRedraw::All,
            window_commands: commands,
            exit: false,
        }
    }

    fn prepare_window(
        &mut self,
        id: WindowId,
        title: &str,
        interactive: bool,
    ) -> Result<WindowCommand, FrameworkError> {
        let document_id = DocumentId::new(id.0 + 1).expect("document id");
        let mut document = RuntimeDocument::new(document_id);
        if id == INPUT {
            self.input_ui = Some(ui::mount_input(&mut document)?);
        } else {
            ui::mount_picture(&mut document)?;
        }
        self.windows.insert(id, Hosted { document });
        Ok(WindowCommand::Open {
            id,
            settings: self.descriptor(id, title, interactive),
        })
    }

    fn descriptor(&self, id: WindowId, title: &str, interactive: bool) -> WindowDescriptor {
        let (width, height, key, shown) = match id {
            INPUT => (
                self.settings.input_width,
                self.settings.input_height,
                "input",
                self.settings.show_input && self.settings.input_presence == InputPresence::Resident,
            ),
            TYPEWRITER => (
                self.settings.typewriter_width,
                self.settings.typewriter_height,
                "typewriter",
                self.settings.show_typewriter,
            ),
            HISTORY => (
                self.settings.history_width,
                self.settings.history_height,
                "history",
                self.settings.show_history,
            ),
            _ => (520.0, 720.0, "settings", true),
        };
        let (x, y) =
            geom::saved_or_default(&self.settings, key, f64::from(width), f64::from(height));
        let mut descriptor = WindowDescriptor::new(title)
            .initial_size(f64::from(width), f64::from(height))
            .minimum_size(280.0, 160.0)
            .surface(WindowSurfacePreference::RequireComposition)
            .shadow(if id == SETTINGS {
                WindowShadow::Auto
            } else {
                WindowShadow::None
            })
            .skip_taskbar(id != SETTINGS)
            .tag(key);
        descriptor.initial_position = Some((x, y));
        descriptor.visible = shown;
        descriptor.always_on_top = id != SETTINGS;
        descriptor.resizable = self.settings.layout_adjust || id == SETTINGS;
        descriptor.role = if id == SETTINGS {
            WindowRole::Main
        } else {
            WindowRole::Tool
        };
        descriptor.focus_on_show = interactive || id == SETTINGS;
        let _ = interactive;
        descriptor
    }

    fn open_settings(&mut self) -> RuntimeProgramUpdate {
        if self.settings_open {
            if let Some(handle) = self.handles.get(&SETTINGS) {
                let _ = handle.focus();
            }
            return RuntimeProgramUpdate::default();
        }
        let document_id = DocumentId::new(SETTINGS.0 + 1).expect("settings document");
        let mut document = RuntimeDocument::new(document_id);
        match ui::mount_settings(&mut document, &self.settings) {
            Ok(form) => self.settings_ui = Some(form),
            Err(error) => {
                self.status = error.to_string();
                return RuntimeProgramUpdate::default();
            }
        }
        self.windows.insert(SETTINGS, Hosted { document });
        self.settings_open = true;
        RuntimeProgramUpdate {
            redraw: nana_ui::RuntimeRedraw::Window(SETTINGS),
            window_commands: vec![WindowCommand::Open {
                id: SETTINGS,
                settings: self.descriptor(SETTINGS, "NanaType 设置", true),
            }],
            exit: false,
        }
    }

    fn apply(&mut self, message: Message) -> bool {
        let remember = message.clone();
        match message {
            Message::Bold => self.draft.map_style(|style| style.bold = !style.bold),
            Message::Italic => self.draft.map_style(|style| style.italic = !style.italic),
            Message::Underline => self
                .draft
                .map_style(|style| style.underline = !style.underline),
            Message::ColorAccent => self.draft.map_style(|style| {
                style.color = Some([220, 64, 72, 255]);
            }),
            Message::ColorBody => self.draft.map_style(|style| style.color = None),
            Message::Larger => {
                let base_px = self.pack().chrome.body_px;
                self.draft.map_style(|style| {
                    let base = style.size_px.unwrap_or(base_px);
                    style.size_px = Some((base + 4.0).min(96.0));
                });
            }
            Message::Smaller => {
                let base_px = self.pack().chrome.body_px;
                self.draft.map_style(|style| {
                    let next = style.size_px.unwrap_or(base_px) - 4.0;
                    style.size_px = (next > 12.0).then_some(next);
                });
            }
            Message::Motion(motion) => self.draft.map_style(|style| style.motion = motion),
            Message::Wait => self.draft.insert_wait(500),
            Message::SpeedMark => self.draft.insert_speed(self.settings.speed_ms.max(1)),
            Message::Instant => self.draft.insert_instant(),
            Message::Pause => self.draft.insert_pause(),
            Message::PickImage => return false,
            Message::PickStickers => return false,
            Message::NextSticker => self.insert_sticker(),
            Message::Paste => self.paste(),
            Message::Submit => self.submit(),
            Message::Theme(id) => {
                self.settings.theme = id;
                self.theme = theme_ui::compiled(&self.pack());
                self.dirty = true;
            }
            Message::ToggleTyping => self.settings.simulate_typing = !self.settings.simulate_typing,
            Message::TogglePop => self.settings.simulate_pop = !self.settings.simulate_pop,
            Message::TogglePerChar => self.settings.per_character = !self.settings.per_character,
            Message::ToggleSound => {
                self.settings.typewriter_sound = !self.settings.typewriter_sound
            }
            Message::ToggleFade => {
                self.settings.after_line = if self.settings.after_line == AfterLine::Hold {
                    AfterLine::Fade
                } else {
                    AfterLine::Hold
                };
            }
            Message::PresenceResident => self.settings.input_presence = InputPresence::Resident,
            Message::PresenceHotkey => {
                self.settings.input_presence = InputPresence::Hotkey;
                self.settings.show_input = false;
            }
            Message::LayoutAdjust => self.settings.layout_adjust = !self.settings.layout_adjust,
            Message::CaptureSummon => self.capture = Some(Capture::Summon),
            Message::CaptureContinue => self.capture = Some(Capture::Continue),
            Message::ShowInput => self.settings.show_input = !self.settings.show_input,
            Message::ShowTypewriter => {
                self.settings.show_typewriter = !self.settings.show_typewriter
            }
            Message::ShowHistory => self.settings.show_history = !self.settings.show_history,
            Message::SpoutTypewriter => {
                self.settings.spout_typewriter = !self.settings.spout_typewriter
            }
            Message::SpoutHistory => self.settings.spout_history = !self.settings.spout_history,
        }
        if !matches!(
            remember,
            Message::Bold
                | Message::Italic
                | Message::Underline
                | Message::ColorAccent
                | Message::ColorBody
                | Message::Larger
                | Message::Smaller
                | Message::Motion(_)
                | Message::Wait
                | Message::SpeedMark
                | Message::Instant
                | Message::Pause
                | Message::NextSticker
                | Message::Paste
                | Message::Submit
        ) {
            self.dirty = true;
        }
        self.refresh_status();
        true
    }

    fn submit(&mut self) {
        if !submittable(&self.draft) {
            return;
        }
        let name = self.settings.speaker_name.clone();
        let submitted = self.draft.clone();
        self.history.push(submitted.clone(), name.clone());
        if self.settings.per_character {
            self.pop_started = None;
        } else {
            self.playback = Some(Playback::start(
                submitted,
                name,
                self.settings.playback_config(),
                self.now_ms(),
            ));
        }
        self.draft.clear();
        self.history_pos = None;
        self.preedit.clear();
        self.input_rev = self.input_rev.wrapping_add(1);
        self.history_rev = self.history_rev.wrapping_add(1);
        self.typewriter_rev = self.typewriter_rev.wrapping_add(1);
        if self.settings.input_presence == InputPresence::Hotkey {
            self.settings.show_input = false;
            self.dirty = true;
        }
    }

    fn paste(&mut self) {
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            return;
        };
        if let Ok(image) = clipboard.get_image() {
            let atom = images::from_rgba(
                image.width as u32,
                image.height as u32,
                image.bytes.into_owned(),
                "剪贴板",
            );
            self.note_insert(true);
            self.draft.insert_image(atom);
            return;
        }
        if let Ok(text) = clipboard.get_text() {
            self.insert_plain(&text);
        }
    }

    fn insert_plain(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if text.is_empty() {
            return;
        }
        let empty = !submittable(&self.draft);
        let graphemes = text.chars().filter(|ch| *ch != '\n').count();
        self.draft.insert_text(&text);
        if empty && submittable(&self.draft) && self.settings.per_character {
            self.pop_started = Some(self.now_ms());
        }
        if self.settings.typewriter_sound && self.settings.per_character {
            for _ in 0..graphemes.min(8) {
                sound::click();
            }
        }
    }

    fn note_insert(&mut self, image: bool) {
        let empty = !submittable(&self.draft);
        if image && empty && self.settings.per_character {
            self.pop_started = Some(self.now_ms());
        }
    }

    fn insert_sticker(&mut self) {
        let dir = self.settings.sticker_dir.clone();
        if dir.is_empty() {
            self.status = "还没有贴纸目录".into();
            return;
        }
        let mut files = Vec::new();
        let Ok(read) = std::fs::read_dir(&dir) else {
            self.status = "打不开贴纸目录".into();
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            ) {
                files.push(path);
            }
        }
        files.sort();
        if files.is_empty() {
            self.status = "贴纸目录是空的".into();
            return;
        }
        let path = &files[self.sticker_index % files.len()];
        self.sticker_index = self.sticker_index.wrapping_add(1);
        match images::from_path(path, ImageKind::Sticker) {
            Ok(atom) => {
                self.note_insert(true);
                self.draft.insert_image(atom);
            }
            Err(error) => self.status = error,
        }
    }

    fn insert_image_file(&mut self, path: &std::path::Path) {
        match images::from_path(path, ImageKind::File) {
            Ok(atom) => {
                self.note_insert(true);
                self.draft.insert_image(atom);
            }
            Err(error) => self.status = error,
        }
    }

    fn recall(&mut self, index: usize) {
        let Some(entry) = self.history.get(index) else {
            return;
        };
        let HistoryEntry { document, .. } = entry;
        self.draft.replace_with(document.clone());
        self.history_pos = Some(index);
        self.input_rev = self.input_rev.wrapping_add(1);
    }

    fn refresh_status(&mut self) {
        let typing = if self.settings.simulate_typing {
            "模拟打字"
        } else {
            "直接显示"
        };
        let after = if self.settings.after_line == AfterLine::Hold {
            "停留"
        } else {
            "淡出"
        };
        let mode = if self.settings.per_character {
            "逐字同步"
        } else {
            "提交播放"
        };
        let sound = if self.settings.typewriter_sound {
            "有打字音"
        } else {
            "无打字音"
        };
        let pop = if self.settings.simulate_pop {
            "有弹出"
        } else {
            "无弹出"
        };
        let presence = if self.settings.input_presence == InputPresence::Resident {
            "输入常驻"
        } else {
            "输入热键"
        };
        let layout = if self.settings.layout_adjust {
            "布局可调"
        } else {
            "点击穿透"
        };
        self.status = format!(
            "{typing} · {after} · {mode} · {sound} · {pop} · {presence} · {layout} · 召唤 {} · 继续 {}",
            self.settings.summon_hotkey.label(),
            self.settings.continue_hotkey.label()
        );
        if !self.typewriter_lane.error().is_empty() {
            self.status = format!("{} · {}", self.status, self.typewriter_lane.error());
        }
        if !self.history_lane.error().is_empty() {
            self.status = format!("{} · {}", self.status, self.history_lane.error());
        }
        let hotkey = self.hotkeys.status();
        if !hotkey.is_empty() {
            self.status = format!("{} · {hotkey}", self.status);
        }
    }

    fn focus_proxy(&mut self) {
        let Some(proxy) = self.input_ui.as_ref().map(|ui| ui.proxy) else {
            return;
        };
        let Some(hosted) = self.windows.get_mut(&INPUT) else {
            return;
        };
        let id = hosted.document.document();
        let _ = hosted
            .document
            .context_mut()
            .focus_node(id, proxy.stable_id());
    }

    fn place_proxy(&mut self, caret: (f32, f32)) {
        let Some(proxy) = self.input_ui.as_ref().map(|ui| ui.proxy) else {
            return;
        };
        let Some(hosted) = self.windows.get_mut(&INPUT) else {
            return;
        };
        let layout = ui::proxy_layout(caret.0, caret.1);
        let _ = hosted
            .document
            .context_mut()
            .update_component(proxy, move |input, _| {
                input.style.layout = layout;
            });
    }

    fn harvest_proxy(&mut self) -> Option<String> {
        let Some(proxy) = self.input_ui.as_ref().map(|ui| ui.proxy) else {
            return None;
        };
        let Some(hosted) = self.windows.get_mut(&INPUT) else {
            return None;
        };
        let document_id = hosted.document.document();
        if hosted
            .document
            .context()
            .focused_text_editor_composing(document_id)
        {
            return None;
        }
        let text = hosted
            .document
            .context_mut()
            .update_component(proxy, |input, _| {
                let text = input.state.value.as_str().to_owned();
                if !text.is_empty() {
                    input.state = TextInputState::new("");
                }
                text
            })
            .unwrap_or_default();
        if text.is_empty() {
            return None;
        }
        self.accept_text(&text);
        Some(text)
    }

    fn harvest_settings(&mut self) {
        let Some(ui) = self.settings_ui.as_ref() else {
            return;
        };
        let fields = (
            ui.name,
            ui.speed,
            ui.fade_base,
            ui.fade_per,
            ui.sticker,
            ui.spout_typewriter,
            ui.spout_history,
            ui.input_w,
            ui.input_h,
            ui.typewriter_w,
            ui.typewriter_h,
            ui.history_w,
            ui.history_h,
        );
        let Some(hosted) = self.windows.get_mut(&SETTINGS) else {
            return;
        };
        let read = |hosted: &mut Hosted, entity: Entity<TextInput>| -> String {
            hosted
                .document
                .context_mut()
                .update_component(entity, |input, _| input.state.value.as_str().to_owned())
                .unwrap_or_default()
        };
        let name = read(hosted, fields.0);
        let speed = read(hosted, fields.1);
        let fade_base = read(hosted, fields.2);
        let fade_per = read(hosted, fields.3);
        let sticker = read(hosted, fields.4);
        let spout_tw = read(hosted, fields.5);
        let spout_hi = read(hosted, fields.6);
        let input_w = read(hosted, fields.7);
        let input_h = read(hosted, fields.8);
        let tw_w = read(hosted, fields.9);
        let tw_h = read(hosted, fields.10);
        let hi_w = read(hosted, fields.11);
        let hi_h = read(hosted, fields.12);
        if name != self.settings.speaker_name {
            self.settings.speaker_name = name;
            self.dirty = true;
        }
        assign_u32(&mut self.settings.speed_ms, &speed, &mut self.dirty);
        assign_u32(&mut self.settings.fade_base_ms, &fade_base, &mut self.dirty);
        assign_u32(
            &mut self.settings.fade_per_char_ms,
            &fade_per,
            &mut self.dirty,
        );
        if sticker != self.settings.sticker_dir {
            self.settings.sticker_dir = sticker;
            self.dirty = true;
        }
        if !spout_tw.is_empty() && spout_tw != self.settings.spout_name_typewriter {
            self.settings.spout_name_typewriter = spout_tw;
            self.dirty = true;
        }
        if !spout_hi.is_empty() && spout_hi != self.settings.spout_name_history {
            self.settings.spout_name_history = spout_hi;
            self.dirty = true;
        }
        assign_f32(&mut self.settings.input_width, &input_w, &mut self.dirty);
        assign_f32(&mut self.settings.input_height, &input_h, &mut self.dirty);
        assign_f32(&mut self.settings.typewriter_width, &tw_w, &mut self.dirty);
        assign_f32(&mut self.settings.typewriter_height, &tw_h, &mut self.dirty);
        assign_f32(&mut self.settings.history_width, &hi_w, &mut self.dirty);
        assign_f32(&mut self.settings.history_height, &hi_h, &mut self.dirty);
        self.relink_edited_sizes();
    }

    /// Keep the input stage and the typewriter client on one rectangle.
    fn apply_region(&mut self, width: f32, height: f32, source: Option<WindowId>) {
        let width = width.max(280.0);
        let height = height.max(120.0);
        let bar = paint::TOOLBAR_PX;
        let input_size = (width, height + bar);
        let tw_size = (width, height);
        let input_changed = (self.settings.input_width - input_size.0).abs() > 1.0
            || (self.settings.input_height - input_size.1).abs() > 1.0;
        let tw_changed = (self.settings.typewriter_width - tw_size.0).abs() > 1.0
            || (self.settings.typewriter_height - tw_size.1).abs() > 1.0;
        self.settings.input_width = input_size.0;
        self.settings.input_height = input_size.1;
        self.settings.typewriter_width = tw_size.0;
        self.settings.typewriter_height = tw_size.1;
        self.region = (width, height);
        if input_changed && source != Some(INPUT) {
            if let Some(handle) = self.handles.get(&INPUT) {
                let _ = handle.set_size((f64::from(input_size.0), f64::from(input_size.1)));
            }
        }
        if tw_changed && source != Some(TYPEWRITER) {
            if let Some(handle) = self.handles.get(&TYPEWRITER) {
                let _ = handle.set_size((f64::from(tw_size.0), f64::from(tw_size.1)));
            }
        }
        if input_changed || tw_changed {
            self.dirty = true;
        }
    }

    fn sync_region_from(&mut self, id: WindowId, geometry: WindowGeometry) {
        let width = geometry.logical_size.0 as f32;
        let height = geometry.logical_size.1 as f32;
        if id == INPUT {
            self.apply_region(width, height - paint::TOOLBAR_PX, Some(INPUT));
        } else if id == TYPEWRITER {
            self.apply_region(width, height, Some(TYPEWRITER));
        }
    }

    fn relink_edited_sizes(&mut self) {
        let bar = paint::TOOLBAR_PX;
        let from_input = (self.settings.input_width, self.settings.input_height - bar);
        let from_tw = (
            self.settings.typewriter_width,
            self.settings.typewriter_height,
        );
        let close =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1.0 && (a.1 - b.1).abs() < 1.0;
        if close(from_input, from_tw) {
            self.region = from_tw;
            return;
        }
        let next = if !close(from_input, self.region) {
            from_input
        } else {
            from_tw
        };
        self.apply_region(next.0, next.1, None);
    }

    fn write_settings_labels(&mut self) {
        let Some(ui) = self.settings_ui.as_ref() else {
            return;
        };
        let summon_entity = ui.summon;
        let continue_entity = ui.continue_key;
        let status_entity = ui.status;
        let summon = format!("召唤热键 {}", self.settings.summon_hotkey.label());
        let continue_key = format!("继续热键 {}", self.settings.continue_hotkey.label());
        let status = self.status.clone();
        let Some(hosted) = self.windows.get_mut(&SETTINGS) else {
            return;
        };
        let _ = hosted
            .document
            .context_mut()
            .update_component(summon_entity, |text, _| text.value = summon);
        let _ = hosted
            .document
            .context_mut()
            .update_component(continue_entity, |text, _| text.value = continue_key);
        let _ = hosted
            .document
            .context_mut()
            .update_component(status_entity, |text, _| text.value = status);
    }

    fn apply_chrome(&mut self) {
        let adjust = self.settings.layout_adjust;
        for (id, shown, passthrough) in [
            (INPUT, self.settings.show_input, false),
            // Click-through (WS_EX_TRANSPARENT) stops the compositor from
            // delivering redraws, so the overlay never paints. Both surfaces
            // stay hittable; layout adjust still toggles resizing.
            (TYPEWRITER, self.settings.show_typewriter, false),
            (HISTORY, self.settings.show_history, false),
        ] {
            if let Some(handle) = self.handles.get(&id) {
                let _ = handle.set_visible(shown);
                let _ = handle.set_resizable(adjust);
                if id != INPUT {
                    let _ = handle.set_mouse_passthrough(passthrough);
                }
            }
        }
    }

    fn save_soon(&mut self) {
        if !self.dirty || self.last_save.elapsed() < Duration::from_millis(400) {
            return;
        }
        if let Err(error) = self.settings.save() {
            self.status = error;
        }
        self.dirty = false;
        self.last_save = Instant::now();
    }

    fn paint_input(&mut self, geometry: WindowGeometry) -> Option<Frame> {
        let chrome = self.pack().chrome;
        let name = self.settings.speaker_name.clone();
        let pieces = self.draft.pieces().to_vec();
        let caret = self.draft.caret();
        let selection = self.draft.selection_range();
        let preedit = self.preedit.clone();
        let elapsed = self.now_ms();
        let scene = Scene {
            width: geometry.logical_size.0.max(1.0),
            height: (geometry.logical_size.1 - paint::TOOLBAR_PX).max(1.0),
            scale: geometry.scale_factor.max(1.0),
            chrome: &chrome,
            name: &name,
            pieces: &pieces,
            show_markers: true,
            caret: Some(caret),
            selection,
            preedit: &preedit,
            opacity: 1.0,
            pop: 1.0,
            blink: (elapsed / 500) % 2 == 0,
            elapsed_ms: elapsed,
        };
        Some(self.painter.dialogue(scene))
    }

    fn paint_typewriter(&mut self, geometry: WindowGeometry) -> Option<Frame> {
        let shown = if self.settings.per_character {
            self.draft.pieces().to_vec()
        } else {
            self.playback
                .as_ref()
                .map(|play| play.revealed().to_vec())
                .unwrap_or_default()
        };
        let name = if self.settings.per_character {
            self.settings.speaker_name.clone()
        } else {
            self.playback
                .as_ref()
                .map(|play| play.name().to_owned())
                .unwrap_or_default()
        };
        let now = self.now_ms();
        let (opacity, pop) = if self.settings.per_character {
            per_char_visual(self.pop_started, now, self.settings.simulate_pop)
        } else if let Some(play) = &self.playback {
            (play.opacity(now), play.scale(now))
        } else {
            (1.0, 1.0)
        };
        let chrome = self.pack().chrome;
        let scene = Scene {
            width: geometry.logical_size.0.max(1.0),
            height: geometry.logical_size.1.max(1.0),
            scale: geometry.scale_factor.max(1.0),
            chrome: &chrome,
            name: &name,
            pieces: &shown,
            show_markers: false,
            caret: None,
            selection: None,
            preedit: "",
            opacity,
            pop,
            blink: false,
            elapsed_ms: now,
        };
        Some(self.painter.dialogue(scene))
    }

    fn paint_history(&mut self, geometry: WindowGeometry) -> Option<Frame> {
        let chrome = self.pack().chrome;
        let elapsed = self.now_ms();
        let requested = if self.history_pinned {
            f32::MAX
        } else {
            self.history_scroll
        };
        let owned: Vec<(usize, String, Vec<Piece>)> = self
            .history
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| (index, entry.name.clone(), entry.document.pieces().to_vec()))
            .collect();
        let lines: Vec<paint::HistoryLine<'_>> = owned
            .iter()
            .map(|(index, name, pieces)| paint::HistoryLine {
                index: *index,
                name,
                pieces,
            })
            .collect();
        let painted = self.painter.history(
            geometry.logical_size.0.max(1.0),
            geometry.logical_size.1.max(1.0),
            geometry.scale_factor.max(1.0),
            &chrome,
            &lines,
            requested,
            elapsed,
        );
        self.history_scroll = painted.scroll;
        self.history_pinned = painted.at_end;
        Some(painted.frame)
    }

    fn on_key(&mut self, key: &nana_ui::KeyInput) -> bool {
        if key.state != KeyState::Pressed {
            return false;
        }
        if let Some(capture) = self.capture.take() {
            if key.physical.0 != "Escape" {
                let hotkey = dialogue::Hotkey {
                    ctrl: key.modifiers.control,
                    shift: key.modifiers.shift,
                    alt: key.modifiers.alt,
                    win: key.modifiers.meta,
                    key: key.physical.0.to_string(),
                };
                match capture {
                    Capture::Summon => self.settings.summon_hotkey = hotkey,
                    Capture::Continue => self.settings.continue_hotkey = hotkey,
                }
                self.dirty = true;
            }
            self.refresh_status();
            return true;
        }
        let physical = key.physical.0.as_ref();
        let logical = key.logical.0.as_ref();
        let ctrl = key.modifiers.control && !key.modifiers.alt;
        if ctrl && matches!(physical, "Enter") || logical == "Enter" && ctrl {
            self.submit();
            return true;
        }
        if physical == "Escape" && self.settings.input_presence == InputPresence::Hotkey {
            self.settings.show_input = false;
            return true;
        }
        if self.continue_pressed(physical) {
            return self.continue_line();
        }
        if ctrl && physical == "KeyA" {
            self.draft.select_all();
            return true;
        }
        if ctrl && physical == "KeyZ" {
            self.draft.undo();
            return true;
        }
        if ctrl && physical == "KeyY" {
            self.draft.redo();
            return true;
        }
        if ctrl && physical == "KeyC" {
            self.copy(false);
            return true;
        }
        if ctrl && physical == "KeyX" {
            self.copy(true);
            return true;
        }
        if ctrl && physical == "KeyV" {
            self.paste();
            return true;
        }
        if ctrl && matches!(physical, "ArrowUp") {
            let index = self
                .history_pos
                .map(|pos| pos.saturating_sub(1))
                .unwrap_or_else(|| self.history.len().saturating_sub(1));
            self.recall(index);
            return true;
        }
        if ctrl && matches!(physical, "ArrowDown") {
            if let Some(pos) = self.history_pos {
                self.recall(
                    pos.saturating_add(1)
                        .min(self.history.len().saturating_sub(1)),
                );
            }
            return true;
        }
        if matches!(physical, "Backspace") {
            self.draft.backspace();
            return true;
        }
        if matches!(physical, "Delete") {
            self.draft.delete_forward();
            return true;
        }
        if matches!(physical, "ArrowLeft") {
            self.draft.move_caret(-1, key.modifiers.shift);
            return true;
        }
        if matches!(physical, "ArrowRight") {
            self.draft.move_caret(1, key.modifiers.shift);
            return true;
        }
        if matches!(physical, "Enter") && logical == "Enter" {
            self.draft.insert_newline();
            return true;
        }
        if matches!(physical, "ArrowUp" | "ArrowDown") {
            self.move_line(physical == "ArrowUp", key.modifiers.shift);
            return true;
        }
        false
    }

    fn continue_pressed(&self, physical: &str) -> bool {
        physical == self.settings.continue_hotkey.key
            && !self.settings.continue_hotkey.ctrl
            && !self.settings.continue_hotkey.alt
            && !self.settings.continue_hotkey.shift
            && !self.settings.continue_hotkey.win
    }

    fn continue_line(&mut self) -> bool {
        if self.last_continue.elapsed() < Duration::from_millis(80) {
            return true;
        }
        self.last_continue = Instant::now();
        let now = self.now_ms();
        if let Some(play) = self.playback.as_mut() {
            play.continue_key(now);
        }
        true
    }

    fn move_line(&mut self, up: bool, selecting: bool) {
        let Some(frame) = &self.input_frame else {
            return;
        };
        let caret = self.draft.caret();
        let Some(current) = frame.slots.iter().find(|slot| slot.index == caret) else {
            return;
        };
        let target_line = if up {
            current.line.saturating_sub(1)
        } else {
            current.line.saturating_add(1)
        };
        let next = frame
            .slots
            .iter()
            .filter(|slot| slot.line == target_line)
            .min_by(|left, right| {
                (left.x - current.x)
                    .abs()
                    .partial_cmp(&(right.x - current.x).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(slot) = next {
            self.draft.set_caret(slot.index, selecting);
        }
    }

    fn copy(&mut self, cut: bool) {
        let text = self.draft.selected_plain_text();
        if text.is_empty() {
            return;
        }
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
        if cut {
            self.draft.insert_text("");
        }
    }

    fn hit_caret(&mut self, x: f32, y: f32, selecting: bool) {
        let Some(frame) = &self.input_frame else {
            return;
        };
        let local_y = y - paint::TOOLBAR_PX;
        let (bx, by, bw, bh) = frame.body;
        if x < bx || local_y < by || x > bx + bw || local_y > by + bh {
            return;
        }
        let next = frame
            .slots
            .iter()
            .min_by(|left, right| {
                let dl = (left.x - x).hypot(left.y - local_y);
                let dr = (right.x - x).hypot(right.y - local_y);
                dl.partial_cmp(&dr).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|slot| slot.index);
        if let Some(index) = next {
            self.draft.set_caret(index, selecting);
        }
    }

    fn hit_history(&mut self, y: f32) {
        let height = self.history_height_l;
        if height <= 1.0 || self.history.is_empty() {
            return;
        }
        let mut y = y;
        if y > height && self.history_scale > 1.0 {
            y /= self.history_scale;
        }
        let index = self.history_frame.as_ref().and_then(|frame| {
            frame
                .rows
                .iter()
                .find(|row| y >= row.top && y < row.bottom)
                .map(|row| row.index)
        });
        if let Some(index) = index {
            self.recall(index);
        }
    }

    fn scroll_history(&mut self, delta_y: f32, unit: WheelUnit) {
        let step = match unit {
            WheelUnit::Lines => delta_y * 72.0,
            WheelUnit::Pixels => delta_y,
        };
        if step == 0.0 {
            return;
        }
        // Wheel away from the user (positive) moves toward older lines.
        self.history_scroll = (self.history_scroll - step).max(0.0);
        self.history_pinned = false;
        self.history_rev = self.history_rev.wrapping_add(1);
    }

    fn file_dialog(&self, id: u64, kind: FileDialogKind) -> WindowCommand {
        let request = FileDialogRequest::new(id, kind)
            .title(if id == DIALOG_IMAGE {
                "插入图片"
            } else {
                "贴纸目录"
            })
            .filters([FileFilter::new(
                "图片",
                ["png", "jpg", "jpeg", "gif", "webp", "bmp"],
            )]);
        WindowCommand::OpenFileDialog { id: INPUT, request }
    }
}

fn submittable(document: &DialogueDocument) -> bool {
    document.pieces().iter().any(|piece| match piece {
        dialogue::Piece::Grapheme { cluster, .. } => cluster != "\n",
        dialogue::Piece::Image(_) => true,
        _ => false,
    })
}

fn assign_u32(slot: &mut u32, text: &str, dirty: &mut bool) {
    if let Ok(value) = text.trim().parse::<u32>()
        && value != *slot
    {
        *slot = value;
        *dirty = true;
    }
}

fn assign_f32(slot: &mut f32, text: &str, dirty: &mut bool) {
    if let Ok(value) = text.trim().parse::<f32>()
        && value >= 80.0
        && (value - *slot).abs() > 0.5
    {
        *slot = value;
        *dirty = true;
    }
}

fn per_char_visual(started: Option<u64>, now: u64, enabled: bool) -> (f32, f32) {
    let Some(started) = started else {
        return (1.0, 1.0);
    };
    let progress = dialogue::pop_progress(now.saturating_sub(started), enabled);
    (progress, dialogue::pop_scale(progress))
}

impl RuntimeProgram for App {
    type Message = Message;
    type Error = String;

    fn gpu_backend_policy() -> GpuBackendPolicy {
        GpuBackendPolicy::CompositionCapable
    }

    fn startup_window_material_mode() -> MaterialEffect {
        MaterialEffect::Solid
    }

    fn initialize(
        _context: &RuntimeProgramContext<Self::Message>,
    ) -> Result<(Self, Vec<Self::Message>), Self::Error> {
        let mut settings = dialogue::Settings::load();
        let sizes_changed = settings.align_dialogue_windows();
        let region = (settings.typewriter_width, settings.typewriter_height);
        let theme = theme_ui::compiled(&ThemePack::get(settings.theme));
        let tray = tray::build()?;
        let hotkeys = Hotkeys::new(
            settings.summon_hotkey.clone(),
            settings.continue_hotkey.clone(),
        );
        let mut windows = HashMap::new();
        let document_id = DocumentId::new(1).expect("root document");
        let mut document = RuntimeDocument::new(document_id);
        ui::mount_root(&mut document).map_err(|error| error.to_string())?;
        windows.insert(WindowId::PRIMARY, Hosted { document });
        let mut app = Self {
            settings,
            theme,
            started: Instant::now(),
            draft: DialogueDocument::new(),
            playback: None,
            history: SessionHistory::default(),
            history_pos: None,
            pop_started: None,
            painter: Painter::new(),
            input_frame: None,
            history_frame: None,
            input_lane: Lane::new(),
            typewriter_lane: Lane::new(),
            history_lane: Lane::new(),
            windows,
            handles: HashMap::new(),
            input_ui: None,
            settings_ui: None,
            children_open: false,
            settings_open: false,
            tray,
            hotkeys,
            capture: None,
            suppress: None,
            preedit: String::new(),
            selecting: false,
            sticker_index: 0,
            last_continue: Instant::now(),
            dirty: sizes_changed,
            last_save: Instant::now(),
            status: String::new(),
            next_wake: Instant::now(),
            input_rev: 0,
            input_painted: 0,
            history_rev: 0,
            history_painted: 0,
            typewriter_rev: 0,
            typewriter_painted: 0,
            history_height_l: 0.0,
            history_scale: 1.0,
            history_scroll: 0.0,
            history_pinned: true,
            region,
            paint_clock: PaintClock::new(),
        };
        app.refresh_status();
        Ok((app, Vec::new()))
    }

    fn with_document<R>(
        &self,
        id: WindowId,
        f: impl FnOnce(&RuntimeDocument) -> R,
    ) -> Result<Option<R>, DocumentAccessError> {
        Ok(self.windows.get(&id).map(|hosted| f(&hosted.document)))
    }

    fn with_document_mut<R>(
        &mut self,
        id: WindowId,
        f: impl FnOnce(&mut RuntimeDocument) -> R,
    ) -> Result<Option<R>, DocumentAccessError> {
        Ok(self
            .windows
            .get_mut(&id)
            .map(|hosted| f(&mut hosted.document)))
    }

    fn update(
        &mut self,
        message: Self::Message,
        _context: &RuntimeProgramContext<Self::Message>,
    ) -> RuntimeProgramUpdate {
        let open_image = matches!(message, Message::PickImage);
        let open_stickers = matches!(message, Message::PickStickers);
        let theme_changed = matches!(message, Message::Theme(_));
        self.apply(message.clone());
        if theme_changed {
            let theme = Arc::clone(&self.theme);
            for hosted in self.windows.values_mut() {
                let _ = hosted
                    .document
                    .context_mut()
                    .set_theme_tokens(Arc::clone(&theme));
            }
        }
        self.focus_proxy();
        self.apply_chrome();
        self.write_settings_labels();
        let mut update = RuntimeProgramUpdate::redraw_all();
        if open_image {
            update
                .window_commands
                .push(self.file_dialog(DIALOG_IMAGE, FileDialogKind::OpenFile));
        }
        if open_stickers {
            update
                .window_commands
                .push(self.file_dialog(DIALOG_STICKER, FileDialogKind::PickFolder));
        }
        update
    }

    fn theme(&self) -> Arc<nana_ui::CompiledTheme> {
        Arc::clone(&self.theme)
    }

    fn window_material_mode_for(&self, id: WindowId) -> MaterialEffect {
        match id {
            INPUT | TYPEWRITER | HISTORY => MaterialEffect::Transparent,
            _ => MaterialEffect::Solid,
        }
    }

    fn host_textures(&self, id: WindowId) -> Option<nana_ui::HostTextureRegistry> {
        Some(match id {
            INPUT => self.input_lane.registry(),
            TYPEWRITER => self.typewriter_lane.registry(),
            HISTORY => self.history_lane.registry(),
            _ => return None,
        })
    }

    fn prepare_window_frame(
        &mut self,
        id: WindowId,
        context: &RuntimeProgramContext<Self::Message>,
    ) {
        let geometry = context.geometry();
        if id == HISTORY {
            self.history_height_l = geometry.logical_size.1.max(1.0);
            self.history_scale = geometry.scale_factor.max(1.0);
        }
        let input_rev = self.input_rev;
        let history_rev = self.history_rev;
        let typewriter_rev = self.typewriter_rev;
        if id == TYPEWRITER && !self.settings.per_character {
            let now = self.now_ms();
            let shown = self
                .playback
                .as_mut()
                .map(|play| play.tick(now))
                .unwrap_or(0);
            if self.settings.typewriter_sound {
                for _ in 0..shown.min(8) {
                    sound::click();
                }
            }
        }
        let (visible, spout) = self.lane_gates(id);
        if !visible && !spout {
            return;
        }
        let frame = match id {
            INPUT => self.paint_input(geometry),
            TYPEWRITER => self.paint_typewriter(geometry),
            HISTORY => self.paint_history(geometry),
            _ => None,
        };
        let Some(frame) = frame else {
            return;
        };
        let lane = match id {
            INPUT => &mut self.input_lane,
            TYPEWRITER => &mut self.typewriter_lane,
            HISTORY => &mut self.history_lane,
            _ => return,
        };
        lane.upload(context.gpu(), &frame.pixels, frame.width, frame.height);
        if spout {
            let name = if id == TYPEWRITER {
                self.settings.spout_name_typewriter.clone()
            } else {
                self.settings.spout_name_history.clone()
            };
            lane.publish(true, &name, &frame.pixels, frame.width, frame.height);
        } else if id != INPUT {
            lane.publish(false, "", &[], 0, 0);
        }
        if id == INPUT {
            let caret = frame.caret;
            self.input_frame = Some(frame);
            self.input_painted = input_rev;
            self.place_proxy(caret);
        } else if id == TYPEWRITER {
            self.typewriter_painted = typewriter_rev;
        } else if id == HISTORY {
            self.history_frame = Some(frame);
            self.history_painted = history_rev;
        }
    }

    fn initialize_window(
        &mut self,
        id: WindowId,
        context: &RuntimeProgramContext<Self::Message>,
    ) -> Result<(), String> {
        let theme = Arc::clone(&self.theme);
        self.with_document_mut(id, |document| {
            document.context_mut().set_theme_tokens(theme).map(|_| ())
        })
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "window document was not initialized".to_string())?
        .map_err(|error| error.to_string())?;
        if matches!(id, INPUT | TYPEWRITER | HISTORY) {
            let lane = match id {
                INPUT => &mut self.input_lane,
                TYPEWRITER => &mut self.typewriter_lane,
                _ => &mut self.history_lane,
            };
            surface::ensure_placeholder(lane, context.gpu());
        }
        Ok(())
    }

    fn window_event(
        &mut self,
        event: WindowEvent,
        context: &RuntimeProgramContext<Self::Message>,
    ) -> RuntimeProgramUpdate {
        match event {
            WindowEvent::Ready { id, geometry } => {
                self.handles.insert(id, context.window());
                if let Some(kind) = geom::kind_of(
                    id,
                    Some(INPUT),
                    Some(TYPEWRITER),
                    Some(HISTORY),
                    Some(SETTINGS),
                ) {
                    geom::remember(&mut self.settings, kind, geometry);
                    self.dirty = true;
                }
                if id == INPUT || id == TYPEWRITER {
                    self.sync_region_from(id, geometry);
                }
                if id == WindowId::PRIMARY {
                    return self.open_children();
                }
                self.apply_chrome();
                if id == INPUT {
                    self.focus_proxy();
                }
                RuntimeProgramUpdate::redraw(id)
            }
            WindowEvent::Moved { id, geometry } | WindowEvent::Resized { id, geometry } => {
                if let Some(kind) = geom::kind_of(
                    id,
                    Some(INPUT),
                    Some(TYPEWRITER),
                    Some(HISTORY),
                    Some(SETTINGS),
                ) {
                    geom::remember(&mut self.settings, kind, geometry);
                    self.dirty = true;
                }
                if id == INPUT || id == TYPEWRITER {
                    self.sync_region_from(id, geometry);
                }
                RuntimeProgramUpdate::redraw(id)
            }
            WindowEvent::CloseRequested { id } if id == WindowId::PRIMARY => {
                RuntimeProgramUpdate::default()
            }
            WindowEvent::CloseRequested { id } if id == SETTINGS => {
                self.settings_open = false;
                RuntimeProgramUpdate {
                    window_commands: vec![WindowCommand::Close(id)],
                    ..RuntimeProgramUpdate::default()
                }
            }
            WindowEvent::CloseRequested { id } => {
                if id == INPUT {
                    self.settings.show_input = false;
                } else if id == TYPEWRITER {
                    self.settings.show_typewriter = false;
                } else if id == HISTORY {
                    self.settings.show_history = false;
                }
                self.apply_chrome();
                self.dirty = true;
                RuntimeProgramUpdate::default()
            }
            WindowEvent::Closed { id } => {
                self.handles.remove(&id);
                if id == SETTINGS {
                    self.windows.remove(&id);
                    self.settings_ui = None;
                    self.settings_open = false;
                }
                RuntimeProgramUpdate::default()
            }
            WindowEvent::FileDialogCompleted { result, .. } => {
                if result.error.is_some() || result.paths.is_empty() {
                    return RuntimeProgramUpdate::default();
                }
                if result.id == DIALOG_IMAGE {
                    self.insert_image_file(&result.paths[0]);
                } else if result.id == DIALOG_STICKER {
                    if let Some(path) = result.paths[0].to_str() {
                        self.settings.sticker_dir = path.to_owned();
                        self.dirty = true;
                    }
                }
                RuntimeProgramUpdate::redraw(INPUT)
            }
            _ => RuntimeProgramUpdate::default(),
        }
    }

    fn input_event(
        &mut self,
        id: WindowId,
        input: RoutedInput<'_>,
        context: &RuntimeProgramContext<Self::Message>,
    ) -> Result<RuntimeProgramUpdate, FrameworkError> {
        // The hidden field and the window text event both observe one key, so
        // a character that was just harvested must not be inserted again.
        let harvested = if id == INPUT {
            self.harvest_proxy()
        } else {
            None
        };
        let mut edited = false;
        match &input.event.payload {
            InputPayload::Composition(CompositionInput::Update { text, .. }) => {
                self.preedit = text.clone();
                edited = true;
            }
            InputPayload::Composition(CompositionInput::Commit(text)) => {
                let text = text.clone();
                self.preedit.clear();
                if harvested.as_deref() != Some(text.as_str()) {
                    self.accept_text(&text);
                }
                edited = true;
            }
            InputPayload::Composition(CompositionInput::End) => {
                self.preedit.clear();
                edited = true;
            }
            InputPayload::Text(text) => {
                let value = text.text.clone();
                if harvested.as_deref() != Some(value.as_str()) {
                    self.accept_text(&value);
                }
                edited = true;
            }
            InputPayload::Key(key) if id == INPUT || self.capture.is_some() => {
                edited = self.on_key(key);
            }
            InputPayload::Pointer(pointer)
                if pointer.phase == PointerPhase::Down && pointer.button == 0 =>
            {
                if id == INPUT && pointer.y >= paint::TOOLBAR_PX {
                    let (bx, by, bw, bh) = self
                        .input_frame
                        .as_ref()
                        .map(|frame| frame.body)
                        .unwrap_or((0.0, 0.0, 0.0, 0.0));
                    let local_y = pointer.y - paint::TOOLBAR_PX;
                    if pointer.x >= bx
                        && local_y >= by
                        && pointer.x <= bx + bw
                        && local_y <= by + bh
                    {
                        self.selecting = true;
                        self.hit_caret(pointer.x, pointer.y, false);
                        edited = true;
                    } else if !input.disposition.handled {
                        let _ = context.window().begin_drag();
                    }
                } else if id == INPUT && pointer.y < paint::TOOLBAR_PX && !input.disposition.handled
                {
                    let _ = context.window().begin_drag();
                } else if id == HISTORY {
                    self.hit_history(pointer.y);
                    edited = true;
                }
            }
            InputPayload::Pointer(pointer)
                if pointer.phase == PointerPhase::Move && self.selecting =>
            {
                if id == INPUT {
                    self.hit_caret(pointer.x, pointer.y, true);
                    edited = true;
                }
            }
            InputPayload::Pointer(pointer) if pointer.phase == PointerPhase::Up => {
                self.selecting = false;
            }
            InputPayload::Wheel(wheel) if id == HISTORY => {
                self.scroll_history(wheel.delta_y, wheel.unit);
                self.request_live_frames();
                return Ok(RuntimeProgramUpdate::redraw(HISTORY));
            }
            _ => {}
        }
        if edited {
            self.input_rev = self.input_rev.wrapping_add(1);
            if self.settings.per_character {
                self.typewriter_rev = self.typewriter_rev.wrapping_add(1);
            }
            self.focus_proxy();
            self.request_live_frames();
        }
        Ok(if edited {
            RuntimeProgramUpdate::redraw_all()
        } else {
            RuntimeProgramUpdate::default()
        })
    }

    fn host_failure(&mut self, _failure: nana_ui::HostFailure) {}

    fn frame_demand(&self, id: WindowId) -> FrameDemand {
        let (visible, spout) = self.lane_gates(id);
        if visible || spout {
            FrameDemand::Continuous(NonZeroU32::new(30).expect("fps"))
        } else {
            FrameDemand::OnDemand
        }
    }

    fn next_wakeup(&self) -> Option<Instant> {
        Some(self.next_wake)
    }

    fn wake(
        &mut self,
        _now: Instant,
        _context: &RuntimeProgramContext<Self::Message>,
    ) -> RuntimeProgramUpdate {
        // A fresh `now + delay` is never due: the host compares it with the
        // instant it sampled before asking. Keep a deadline this call can miss.
        self.next_wake = Instant::now() + Duration::from_millis(16);
        self.suppress = None;
        self.harvest_settings();
        let suspended = self.capture.is_some();
        let hits = self.hotkeys.poll(
            &self.settings.summon_hotkey,
            &self.settings.continue_hotkey,
            suspended,
        );
        for hit in hits {
            match hit {
                HotkeyHit::Summon => {
                    self.settings.show_input = true;
                    self.input_rev = self.input_rev.wrapping_add(1);
                    if let Some(handle) = self.handles.get(&INPUT) {
                        let _ = handle.set_visible(true);
                    }
                    // The hotkey thread already called SetForegroundWindow.
                    // A later focus() injects Alt and can hand the foreground back.
                    self.focus_proxy();
                }
                HotkeyHit::Continue => {
                    let _ = self.continue_line();
                }
            }
        }
        if let Some(action) = tray::poll(&self.tray) {
            tray::sync_labels(
                &self.tray,
                self.settings.show_input,
                self.settings.show_typewriter,
                self.settings.show_history,
            );
            match action {
                TrayAction::ToggleInput => self.settings.show_input = !self.settings.show_input,
                TrayAction::ToggleTypewriter => {
                    self.settings.show_typewriter = !self.settings.show_typewriter
                }
                TrayAction::ToggleHistory => {
                    self.settings.show_history = !self.settings.show_history
                }
                TrayAction::Settings => {
                    self.apply_chrome();
                    self.dirty = true;
                    self.save_soon();
                    return self.open_settings();
                }
                TrayAction::Quit => return RuntimeProgramUpdate::exit(),
            }
            self.dirty = true;
        }
        tray::sync_labels(
            &self.tray,
            self.settings.show_input,
            self.settings.show_typewriter,
            self.settings.show_history,
        );
        self.apply_chrome();
        self.refresh_status();
        self.write_settings_labels();
        self.save_soon();
        self.request_live_frames();
        RuntimeProgramUpdate::redraw_all()
    }
}

impl App {
    fn accept_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.suppress.as_deref() == Some(text) {
            self.suppress = None;
            return;
        }
        self.insert_plain(text);
        self.suppress = Some(text.to_owned());
    }

    fn publish_input_hwnd(&self) {
        #[cfg(windows)]
        if let Some(hwnd) = find_hwnd("输入") {
            self.hotkeys.set_input_hwnd(hwnd.0 as isize);
        }
    }

    /// Window visibility and Spout stay independent. A hidden lane can still publish.
    fn lane_gates(&self, id: WindowId) -> (bool, bool) {
        match id {
            INPUT => (self.settings.show_input, false),
            TYPEWRITER => (
                self.settings.show_typewriter,
                self.settings.spout_typewriter,
            ),
            HISTORY => (self.settings.show_history, self.settings.spout_history),
            _ => (false, false),
        }
    }

    fn surface_dirty(&self, id: WindowId) -> bool {
        let (visible, spout) = self.lane_gates(id);
        let live = visible || spout;
        match id {
            INPUT => visible && self.input_rev != self.input_painted,
            HISTORY => live && self.history_rev != self.history_painted,
            TYPEWRITER => {
                live && (self.typewriter_rev != self.typewriter_painted
                    || (!self.settings.per_character
                        && self
                            .playback
                            .as_ref()
                            .is_some_and(|play| play.wants_frame(self.now_ms()))))
            }
            _ => false,
        }
    }

    /// Ask the compositor windows for another frame. `WM_PAINT` stops arriving
    /// on its own once a window is created with `WS_EX_NOREDIRECTIONBITMAP`,
    /// so the playback clock has to invalidate them.
    fn request_live_frames(&mut self) {
        self.publish_input_hwnd();
        let mut bits = 0u8;
        if self.surface_dirty(INPUT) {
            bits |= PAINT_INPUT;
            invalidate_title("输入");
        }
        if self.surface_dirty(TYPEWRITER) {
            bits |= PAINT_TYPEWRITER;
            invalidate_title("打字机");
        }
        if self.surface_dirty(HISTORY) {
            bits |= PAINT_HISTORY;
            invalidate_title("历史");
        }
        self.paint_clock.request(bits);
    }
}

const PAINT_INPUT: u8 = 1;
const PAINT_TYPEWRITER: u8 = 2;
const PAINT_HISTORY: u8 = 4;

struct PaintClock {
    flags: Arc<AtomicU8>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PaintClock {
    fn new() -> Self {
        let flags = Arc::new(AtomicU8::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let flags_thread = Arc::clone(&flags);
        let stop_thread = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("nanatype-paint".into())
            .spawn(move || paint_clock_loop(flags_thread, stop_thread))
            .ok();
        Self {
            flags,
            stop,
            thread,
        }
    }

    fn request(&self, bits: u8) {
        if bits != 0 {
            self.flags.fetch_or(bits, Ordering::Release);
        }
    }
}

impl Drop for PaintClock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // The paint thread's UpdateWindow sends WM_PAINT back to this thread.
        // Waiting for it here deadlocks.
        if let Some(thread) = self.thread.take() {
            std::mem::forget(thread);
        }
    }
}

fn paint_clock_loop(flags: Arc<AtomicU8>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Acquire) {
        let bits = flags.swap(0, Ordering::AcqRel);
        if bits == 0 {
            std::thread::sleep(Duration::from_millis(4));
            continue;
        }
        // Leave the UI callback first. UpdateWindow delivers WM_PAINT immediately,
        // and doing that inside wake would borrow the program twice.
        std::thread::sleep(Duration::from_millis(8));
        if stop.load(Ordering::Acquire) {
            break;
        }
        #[cfg(windows)]
        {
            if bits & PAINT_INPUT != 0 {
                poke_window("输入");
            }
            if bits & PAINT_TYPEWRITER != 0 {
                poke_window("打字机");
            }
            if bits & PAINT_HISTORY != 0 {
                poke_window("历史");
            }
        }
        #[cfg(not(windows))]
        let _ = bits;
    }
}

#[cfg(windows)]
fn find_hwnd(title: &str) -> Option<windows::Win32::Foundation::HWND> {
    let name = windows::core::HSTRING::from(title);
    unsafe { windows::Win32::UI::WindowsAndMessaging::FindWindowW(None, &name) }.ok()
}

#[cfg(windows)]
fn invalidate_title(title: &str) {
    use windows::Win32::Graphics::Gdi::{
        InvalidateRect, RDW_INTERNALPAINT, RDW_INVALIDATE, RedrawWindow,
    };
    let Some(hwnd) = find_hwnd(title) else {
        return;
    };
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
        let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE | RDW_INTERNALPAINT);
    }
}

#[cfg(not(windows))]
fn invalidate_title(_title: &str) {}

#[cfg(windows)]
fn poke_window(title: &str) {
    use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
    let Some(hwnd) = find_hwnd(title) else {
        return;
    };
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
        let _ = UpdateWindow(hwnd);
    }
}
