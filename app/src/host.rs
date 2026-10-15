//! The NanaUI host program: owns one document per window and routes window
//! events, input and messages to [`App`].

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use nana_ui::runtime::view::{entity_ref, with_refs};
use nana_ui::runtime::{
    DocumentId, Entity, FrameworkError, RichTextEditor, RuntimeDocument, Toast, ToastDismissed,
    ToastStack,
};
use nana_ui::{
    CompiledTheme, DocumentAccessError, FrameDemand, GpuBackendPolicy, MaterialEffect, RoutedInput,
    RuntimeProgram, RuntimeProgramContext, RuntimeProgramUpdate,
};
use nana_ui_platform::host::WindowCommand;
use nana_ui_platform::{InputPayload, KeyState, WindowEvent, WindowId};
use nanatype_core::Cmd;
use nanatype_model::Overflow;
use nanatype_settings::{Hotkey, InputPresence};

use crate::app::{App, SEND_KEY};
use crate::clock;
use crate::msg::{Msg, Tool};
use crate::ui::overflow::{self, LineSpan, Measure, PageKey};
use crate::ui::{danmaku, dialogue, history, input, settings};
use crate::windows::WindowKind;

pub struct Host {
    app: App,
    perf: Option<crate::perf::Recorder>,
    docs: HashMap<WindowId, RuntimeDocument>,
    /// The input window's rich editor.
    editor: Option<Entity<RichTextEditor>>,
    source: Option<Entity<nana_ui::runtime::TextArea>>,
    /// Skin and reduced-motion the editor's effect preview was built for.
    editor_fx: Option<(usize, bool, usize)>,
    /// The typewriter's scroll through long text.
    follow: overflow::Follow,
    peer_follow: overflow::Follow,
    /// The typewriter's automatic page breaks.
    pager: overflow::Pager,
    /// The toast stack of each control window (settings, input, chat).
    toasts: HashMap<WindowId, Entity<ToastStack>>,
}

/// The tag every document shapes its text in by default, so Han characters
/// take the active language's glyph shapes instead of the font's first guess.
fn default_language(tag: &str) -> Option<nana_ui::runtime::LanguageTag> {
    nana_ui::runtime::LanguageTag::new(tag)
}

fn is_modifier(code: &str) -> bool {
    matches!(
        code,
        "ShiftLeft"
            | "ShiftRight"
            | "ControlLeft"
            | "ControlRight"
            | "AltLeft"
            | "AltRight"
            | "MetaLeft"
            | "MetaRight"
            | "OSLeft"
            | "OSRight"
    )
}

impl Host {
    fn finish(&mut self) -> RuntimeProgramUpdate {
        self.drive_editor();
        let language = self.app.ui.locale.get_untracked().tag();
        // The app's reduce-motion setting wins over the system's in every
        // window's runtime, so view transitions and toasts land at once too.
        let forced = self.app.settings.playback.reduced_motion;
        for doc in self.docs.values_mut() {
            let cx = doc.context_mut();
            cx.set_default_language(default_language(language));
            if forced {
                cx.set_reduced_motion(true);
            } else {
                cx.follow_system_reduced_motion();
            }
        }
        self.show_toasts();
        if self.app.theme_changed {
            self.app.theme_changed = false;
            let theme = self.app.theme.clone();
            for doc in self.docs.values_mut() {
                let _ = doc.context_mut().set_theme_tokens(theme.clone());
            }
        }
        let mut result = self.app.update_result();
        result.redraw = pending_redraw(&self.docs);
        result
    }

    /// Apply the app's editor commands, and keep the editor's per-glyph
    /// effect preview on the skin's effect table.
    fn drive_editor(&mut self) {
        let commands = std::mem::take(&mut self.app.rich_commands);
        let (Some(editor), Some(doc)) = (self.editor, self.docs.get_mut(&WindowKind::Input.id()))
        else {
            return;
        };
        let document_id = doc.document();
        let cx = doc.context_mut();
        if std::mem::take(&mut self.app.focus_editor) {
            let target = if self.app.ui.source_mode.get_untracked() {
                self.source.map(|s| s.stable_id())
            } else {
                Some(editor.stable_id())
            };
            if let Some(target) = target {
                let _ = cx.focus_node(document_id, target);
            }
        }
        for command in commands {
            if let Err(e) = cx.rich_edit(editor, command) {
                crate::debug_log(&format!("rich edit: {e}"));
            }
        }
        let reduced = self.app.ui.reduced_motion.get_untracked();
        let key = (
            Arc::as_ptr(&self.app.skin) as usize,
            reduced,
            crate::ui::rich::attr_version(),
        );
        if self.editor_fx != Some(key) {
            let effects = crate::ui::rich::effect_table(&self.app.skin.effects, reduced);
            if cx
                .set_rich_presentation_chains(editor.stable_id(), effects, None)
                .is_ok()
            {
                self.editor_fx = Some(key);
            }
        }
    }

    /// Push the app's new toasts into the settings window (when open) and
    /// the input window (while it is visible). A toast that times out or is
    /// closed comes back as [`Msg::ToastDismissed`].
    fn show_toasts(&mut self) {
        let requests = std::mem::take(&mut self.app.toasts);
        if requests.is_empty() {
            return;
        }
        let input_visible = self.app.input_visible();
        for request in requests {
            let mut shown = false;
            for kind in [WindowKind::Settings, WindowKind::Input, WindowKind::Chat] {
                let wanted = match kind {
                    WindowKind::Input => input_visible,
                    WindowKind::Chat => true,
                    _ => request.settings,
                };
                let id = kind.id();
                let (true, Some(stack), Some(doc)) = (
                    wanted,
                    self.toasts.get(&id).copied(),
                    self.docs.get_mut(&id),
                ) else {
                    continue;
                };
                let toast = Toast::new(request.title.as_str(), request.tone)
                    .timeout(std::time::Duration::from_millis(request.ms))
                    .dismissible(true);
                let cx = doc.context_mut();
                let pushed = cx.push_toast(stack, toast).and_then(|toast| {
                    cx.on(toast, move |_, _: &ToastDismissed, vcx| {
                        vcx.dispatch_program_all(Msg::ToastDismissed(kind, toast));
                    })
                });
                match pushed {
                    Ok(()) => shown = true,
                    Err(e) => crate::debug_log(&format!("toast: {e}")),
                }
            }
            // No window to show it in yet (a notice at start-up): keep it
            // for one that opens while it is still fresh.
            if !shown && request.at.elapsed() < std::time::Duration::from_millis(request.ms) {
                self.app.toasts.push(request);
            }
        }
    }

    fn dismiss_toast(&mut self, kind: WindowKind, toast: Entity<Toast>) {
        let id = kind.id();
        if let (Some(stack), Some(doc)) = (self.toasts.get(&id).copied(), self.docs.get_mut(&id)) {
            let _ = doc.context_mut().remove_toast(stack, toast);
        }
    }

    fn mount(&mut self, id: WindowId, kind: WindowKind) -> Result<(), String> {
        let doc_id = DocumentId::new(id.0 + 1).ok_or("document id")?;
        let mut doc = RuntimeDocument::new(doc_id);
        let ui = self.app.ui;
        let context = doc.context_mut();
        let _ = context.set_theme_tokens(self.app.theme.clone());
        let err = |e: FrameworkError| e.to_string();
        match kind {
            WindowKind::Display => {
                context
                    .mount_view_root(doc_id, move || dialogue::display_window(ui))
                    .map_err(err)?;
            }
            WindowKind::PeerDisplay => {
                context
                    .mount_view_root(doc_id, move || dialogue::peer_window(ui))
                    .map_err(err)?;
            }
            WindowKind::History => {
                context
                    .mount_view_root(doc_id, move || history::history_window(ui))
                    .map_err(err)?;
            }
            WindowKind::Settings => {
                let (_, stack) = context
                    .mount_view_root(doc_id, move || {
                        let stack = entity_ref::<ToastStack>();
                        let view =
                            crate::ui::feedback::with_toasts(settings::settings_window(ui), stack);
                        with_refs(view, stack)
                    })
                    .map_err(err)?;
                self.toasts.insert(id, stack);
            }
            WindowKind::Chat => {
                let (_, stack) = context
                    .mount_view_root(doc_id, move || {
                        let stack = entity_ref::<ToastStack>();
                        with_refs(
                            crate::ui::feedback::with_toasts_limit(
                                danmaku::chat_window(ui),
                                stack,
                                1,
                            ),
                            stack,
                        )
                    })
                    .map_err(err)?;
                self.toasts.insert(id, stack);
            }
            WindowKind::Input => {
                let (_, (editor, source, stack)) = context
                    .mount_view_root(doc_id, move || input::input_window(ui))
                    .map_err(err)?;
                context
                    .on_key(editor, key_policy(self.app.ctx.clone(), false))
                    .map_err(err)?;
                context
                    .on_key(source, key_policy(self.app.ctx.clone(), true))
                    .map_err(err)?;
                self.editor = Some(editor);
                self.source = Some(source);
                self.editor_fx = None;
                self.toasts.insert(id, stack);
            }
        }
        doc.context_mut()
            .set_default_language(default_language(self.app.ui.locale.get_untracked().tag()));
        self.docs.insert(id, doc);
        Ok(())
    }

    /// Read the typewriter's last layout: scroll along with the typing, or
    /// tell the core where a full box turns the page.
    fn follow_track(&mut self, now: u64, remote: bool) {
        let ui = if remote {
            self.app.ui.remote()
        } else {
            self.app.ui
        };
        let view = ui.display.get_untracked();
        let mut mode = if remote {
            Overflow::Scroll
        } else {
            self.app.skin.dialogue.overflow
        };
        let mirror = self.app.settings.general.input_mode == nanatype_settings::InputMode::Mirror;
        if mode == Overflow::Page && (mirror || view.line.is_none()) {
            // Nothing to page while the text follows the editor.
            mode = Overflow::Scroll;
        }
        let key = (!view.empty && mode != Overflow::Clip).then(|| PageKey::of(&view));
        let page = overflow::published_on(ui.track_slot).filter(|p| Some(p.key) == key);
        let measure = page.as_ref().and_then(|page| {
            let world = self.docs.get(&ui.display_kind().id())?.context().world();
            let (_, layout) = world.text_layout(ui.text_node.get_untracked()?)?;
            // A layout of an older node until the new one is laid out.
            let end = layout.lines.last()?.source.end;
            if end < page.text_trimmed || end > page.text_len {
                return None;
            }
            Some(Measure {
                lines: layout
                    .lines
                    .iter()
                    .map(|l| LineSpan {
                        bytes: l.source.clone(),
                        top: l.bounds.y,
                        bottom: l.bounds.y + l.bounds.height,
                    })
                    .collect(),
                viewport: world.layout_box(ui.text_port.get_untracked()?)?.height,
                glyphs: &page.glyphs,
            })
        });
        let reduced = ui.reduced_motion.get_untracked();
        let (follow, pager) = match mode {
            Overflow::Page => (None, key),
            _ => (key, None),
        };
        let follower = if remote {
            &mut self.peer_follow
        } else {
            &mut self.follow
        };
        let y = follower.frame(follow, measure.as_ref(), now, reduced);
        if !remote
            && let Some(breaks) = self.pager.frame(pager, measure.as_ref())
            && let Some(line) = view.line
        {
            self.app.post(Msg::Core(Cmd::PageBreaks { line, breaks }));
        }
        if ui.text_scroll.get_untracked() != y {
            ui.text_scroll.set(y);
        }
    }

    fn open_overlays(&mut self) {
        for kind in [WindowKind::Display, WindowKind::History] {
            if !self.app.windows.is_open(kind) {
                let title = self.app.ui.now_t(kind.title_key());
                let desc =
                    crate::windows::descriptor(kind, &title, &self.app.skin, &self.app.settings);
                self.app.commands.push(WindowCommand::Open {
                    id: kind.id(),
                    settings: desc,
                });
            }
        }
        // The chat panel comes back if it was open at quit.
        if self.app.settings.integrations.blivechat.panel_open {
            self.app.open_chat();
        }
    }
}

/// A history row is in its fade-out window.
fn history_fading(ui: crate::ui::Ui, now: u64) -> bool {
    let Some(after) = ui.settings.with_untracked(|s| s.history.fade_after_s) else {
        return false;
    };
    let unix = crate::clock::unix_of(now);
    let start = u64::from(after) * 1000;
    ui.history.with_untracked(|rows| {
        rows.iter().any(|r| {
            let age = unix.saturating_sub(r.item.at);
            age >= start.saturating_sub(100) && age <= start + 700
        })
    })
}

/// Monotonic time at which the next history row starts to fade.
fn next_history_fade(ui: crate::ui::Ui, now: u64) -> Option<u64> {
    let after = u64::from(ui.settings.with_untracked(|s| s.history.fade_after_s)?) * 1000;
    let unix_now = crate::clock::unix_of(now);
    ui.history.with_untracked(|rows| {
        rows.iter()
            .map(|r| r.item.at + after)
            .filter(|t| *t > unix_now)
            .min()
            .map(|t| now + (t - unix_now))
    })
}

/// An image on the clipboard with no text beside it (text pastes normally).
fn clipboard_image() -> Option<Msg> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    if clipboard.get_text().is_ok_and(|t| !t.is_empty()) {
        return None;
    }
    let image = clipboard.get_image().ok()?;
    Some(Msg::PasteImage {
        width: image.width as u32,
        height: image.height as u32,
        rgba: image.bytes.into_owned(),
    })
}

/// The input window's keys before the editor's own editing: send, recall,
/// hide and the formatting shortcuts. The markup `source` field also pastes
/// clipboard images (the rich editor asks for that itself).
fn key_policy(
    post: RuntimeProgramContext<Msg>,
    source: bool,
) -> impl FnMut(&nana_ui::KeyInput) -> bool + Send + 'static {
    move |key| {
        if key.state != KeyState::Pressed {
            return false;
        }
        let m = key.modifiers;
        let logical = key.logical.0.as_ref();
        let ctrl_enter = SEND_KEY.load(Ordering::Relaxed) == 1;
        let plain = m.control && !m.shift && !m.alt;
        let msg = match logical {
            "Enter" if !ctrl_enter && !m.shift && !m.control => Msg::Core(Cmd::Send),
            "Enter" if ctrl_enter && m.control => Msg::Core(Cmd::Send),
            "ArrowUp" if m.control => Msg::Core(Cmd::Recall(-1)),
            "ArrowDown" if m.control => Msg::Core(Cmd::Recall(1)),
            "Escape" => Msg::HideInput,
            "b" | "B" if plain => Msg::Tool(Tool::Bold),
            "i" | "I" if plain => Msg::Tool(Tool::Italic),
            "u" | "U" if plain => Msg::Tool(Tool::Underline),
            "v" | "V" if plain && source => match clipboard_image() {
                Some(msg) => msg,
                None => return false,
            },
            _ => return false,
        };
        post.dispatch(msg);
        true
    }
}

impl RuntimeProgram for Host {
    type Message = Msg;
    type Error = String;

    fn initialize(context: &RuntimeProgramContext<Msg>) -> Result<(Self, Vec<Msg>), String> {
        let app = App::new(context);
        let mut host = Self {
            app,
            perf: crate::perf::Recorder::from_env(),
            docs: HashMap::new(),
            editor: None,
            source: None,
            editor_fx: None,
            follow: overflow::Follow::default(),
            peer_follow: overflow::Follow::default(),
            pager: overflow::Pager::default(),
            toasts: HashMap::new(),
        };
        // The primary window exists before `initialize_window` is ever
        // called, so its document is built here.
        host.mount(WindowKind::Input.id(), WindowKind::Input)?;
        Ok((host, Vec::new()))
    }

    fn with_document<R>(
        &self,
        id: WindowId,
        f: impl FnOnce(&RuntimeDocument) -> R,
    ) -> Result<Option<R>, DocumentAccessError> {
        Ok(self.docs.get(&id).map(f))
    }

    fn with_document_mut<R>(
        &mut self,
        id: WindowId,
        f: impl FnOnce(&mut RuntimeDocument) -> R,
    ) -> Result<Option<R>, DocumentAccessError> {
        Ok(self.docs.get_mut(&id).map(f))
    }

    fn update(&mut self, message: Msg, _: &RuntimeProgramContext<Msg>) -> RuntimeProgramUpdate {
        if let (Some(source), Some(doc)) = (self.source, self.docs.get(&WindowKind::Input.id())) {
            input::capture_source_selection(self.app.ui, doc.context(), source);
        }
        match message {
            Msg::ToastDismissed(kind, toast) => self.dismiss_toast(kind, toast),
            message => self.app.update(message),
        }
        self.finish()
    }

    fn gpu_backend_policy() -> GpuBackendPolicy {
        GpuBackendPolicy::CompositionCapable
    }

    fn theme(&self) -> Arc<CompiledTheme> {
        self.app.theme.clone()
    }

    fn window_material_mode_for(&self, id: WindowId) -> MaterialEffect {
        WindowKind::from_id(id)
            .map(WindowKind::material)
            .unwrap_or(MaterialEffect::Solid)
    }

    fn appearance_backdrop_opacity_for(&self, id: WindowId) -> f32 {
        match WindowKind::from_id(id) {
            Some(WindowKind::Settings | WindowKind::Chat) => 1.0,
            _ => 0.0,
        }
    }

    fn initialize_window(
        &mut self,
        id: WindowId,
        _: &RuntimeProgramContext<Msg>,
    ) -> Result<(), String> {
        let kind = WindowKind::from_id(id).ok_or_else(|| format!("unknown window {id:?}"))?;
        if self.docs.contains_key(&id) {
            return Ok(());
        }
        self.mount(id, kind)
    }

    fn discard_window(&mut self, id: WindowId) {
        self.docs.remove(&id);
        self.toasts.remove(&id);
    }

    fn window_event(
        &mut self,
        event: WindowEvent,
        context: &RuntimeProgramContext<Msg>,
    ) -> RuntimeProgramUpdate {
        match event {
            WindowEvent::Ready { id, geometry } => {
                if let Some(kind) = WindowKind::from_id(id) {
                    let handle = context.window();
                    self.app.windows.ready(kind, handle.clone());
                    if kind == WindowKind::Chat {
                        self.app.chat_resized(geometry.logical_size.0);
                    }
                    if matches!(kind, WindowKind::Settings | WindowKind::Chat) {
                        self.app.tint_captions();
                    }
                    self.app
                        .windows
                        .moved(kind, geometry.logical_position, geometry.logical_size);
                    if kind == WindowKind::Input {
                        crate::debug_log("input window ready");
                        self.open_overlays();
                        let post = self.app.ctx.clone();
                        std::thread::spawn(move || {
                            let hwnd = handle
                                .with_native_handle(|h| match h.as_raw() {
                                    #[cfg(windows)]
                                    raw_window_handle::RawWindowHandle::Win32(w) => w.hwnd.get(),
                                    #[cfg(target_os = "macos")]
                                    raw_window_handle::RawWindowHandle::AppKit(w) => {
                                        // The platform handle is the content view. Activation
                                        // resolves its window on the main thread.
                                        w.ns_view.as_ptr() as isize
                                    }
                                    _ => 0,
                                })
                                .wait()
                                .unwrap_or(0);
                            post.dispatch(Msg::InputHwnd(hwnd));
                        });
                    }
                }
            }
            WindowEvent::Moved { id, geometry } | WindowEvent::Resized { id, geometry } => {
                if let Some(kind) = WindowKind::from_id(id) {
                    self.app
                        .windows
                        .moved(kind, geometry.logical_position, geometry.logical_size);
                    // A drag-resize in layout mode becomes the window's size.
                    let (w, h) = geometry.logical_size;
                    if kind == WindowKind::Chat {
                        self.app.chat_resized(w);
                    }
                    let editing = kind
                        .overlay(&self.app.settings)
                        .is_some_and(|o| o.lock == nanatype_settings::Lock::EditLayout);
                    let current = self.app.window_size(kind);
                    if editing
                        && matches!(
                            kind,
                            WindowKind::Display | WindowKind::PeerDisplay | WindowKind::History
                        )
                        && ((current.0 - w).abs() > 1.0 || (current.1 - h).abs() > 1.0)
                    {
                        self.app.edit(|s| {
                            if let Some(o) = kind.overlay_mut(s) {
                                o.size = Some([w, h]);
                            }
                        });
                    }
                }
            }
            WindowEvent::CloseRequested { id } => match WindowKind::from_id(id) {
                Some(WindowKind::Chat) => {
                    self.app.close_chat();
                }
                Some(WindowKind::Settings) | None => {
                    return RuntimeProgramUpdate {
                        window_commands: vec![WindowCommand::Close(id)],
                        ..RuntimeProgramUpdate::default()
                    };
                }
                Some(WindowKind::Input) => {
                    if self.app.settings.general.input_presence == InputPresence::Summon {
                        self.app.summoned = false;
                    } else {
                        self.app.edit(|s| s.overlays.input.visible = false);
                    }
                }
                Some(kind) => self.app.edit(|s| {
                    if let Some(o) = kind.overlay_mut(s) {
                        o.visible = false;
                    }
                }),
            },
            WindowEvent::Closed { id } => {
                if let Some(kind) = WindowKind::from_id(id) {
                    self.app.windows.closed(kind);
                }
                self.docs.remove(&id);
                self.toasts.remove(&id);
            }
            WindowEvent::FileDialogCompleted { result, .. } => {
                self.app.file_chosen(result.id, result.paths);
            }
            WindowEvent::AppearanceChanged { appearance, .. } => {
                self.app.system_dark =
                    matches!(appearance, nana_ui_platform::SystemAppearance::Dark);
                if self.app.settings.skin.chrome == nanatype_settings::ChromeMode::System {
                    self.app.refresh_theme();
                }
            }
            WindowEvent::ReducedMotionChanged { reduced, .. } => {
                self.app.ui.system_reduced_motion.set(reduced);
                self.app.sync_reduced_motion();
            }
            _ => {}
        }
        self.finish()
    }

    fn input_event(
        &mut self,
        id: WindowId,
        input: RoutedInput<'_>,
        _: &RuntimeProgramContext<Msg>,
    ) -> Result<RuntimeProgramUpdate, FrameworkError> {
        if let InputPayload::Pointer(p) = &input.event.payload
            && p.phase == nana_ui_platform::PointerPhase::Down
            && p.button == 0
            && !input.disposition.handled
        {
            // Overlays have no title bar: in layout mode the interior is a
            // drag handle. A press on the resize band never arrives here.
            let kind = WindowKind::from_id(id);
            let editing = kind
                .and_then(|k| k.overlay(&self.app.settings))
                .is_some_and(|o| o.lock == nanatype_settings::Lock::EditLayout);
            let drag = match kind {
                Some(WindowKind::Display | WindowKind::PeerDisplay | WindowKind::History) => {
                    editing
                }
                Some(WindowKind::Input) => input.pointer_hit.is_none() || editing,
                _ => false,
            };
            if drag {
                return Ok(RuntimeProgramUpdate {
                    window_commands: vec![WindowCommand::Drag(id)],
                    ..RuntimeProgramUpdate::default()
                });
            }
        }
        let InputPayload::Key(key) = &input.event.payload else {
            return Ok(RuntimeProgramUpdate::default());
        };
        if key.state != KeyState::Pressed {
            return Ok(RuntimeProgramUpdate::default());
        }
        if self.app.ui.recording.get_untracked().is_some() {
            let code = key.physical.0.as_ref();
            if code == "Escape" {
                self.app.update(Msg::RecordHotkey(None));
                return Ok(self.finish());
            }
            if !is_modifier(code) {
                let m = key.modifiers;
                let hotkey = Hotkey {
                    ctrl: m.control,
                    shift: m.shift,
                    alt: m.alt,
                    meta: m.meta,
                    key: code.to_owned(),
                };
                if !hotkey.is_safe_global() {
                    let text = self.app.ui.now_t("hotkeys-unsafe");
                    self.app.notice(text, true);
                    return Ok(self.finish());
                }
                self.app.update(Msg::HotkeyCaptured(hotkey));
                return Ok(self.finish());
            }
        }
        if WindowKind::from_id(id) == Some(WindowKind::Input)
            && key.logical.0 == "Escape"
            && !input.disposition.handled
        {
            self.app.update(Msg::HideInput);
            return Ok(self.finish());
        }
        Ok(RuntimeProgramUpdate::default())
    }

    fn scene_gpu_renderers(&self, _: WindowId) -> Option<nana_ui::SceneGpuRendererRegistry> {
        Some(crate::ui::sweep::renderers())
    }

    fn window_output(&self, id: WindowId) -> Option<nana_ui::WindowOutputConfig> {
        let kind = WindowKind::from_id(id)?;
        if kind == WindowKind::PeerDisplay && self.app.peer.handle.is_none() {
            return None;
        }
        crate::output::config(kind, &self.app.settings)
    }

    fn window_frame_presented(
        &mut self,
        id: WindowId,
        _: &RuntimeProgramContext<Msg>,
    ) -> RuntimeProgramUpdate {
        if let Some(perf) = &mut self.perf {
            perf.frame(id, false, self.docs.get(&id));
        }
        RuntimeProgramUpdate::default()
    }

    fn window_output_frame(
        &mut self,
        id: WindowId,
        frame: &nana_ui::WindowOutputFrame,
        context: &RuntimeProgramContext<Msg>,
    ) {
        if self.window_output(id).is_none() {
            return;
        }
        if let Some(perf) = &mut self.perf {
            perf.frame(id, true, self.docs.get(&id));
        }
        let Some(kind) = WindowKind::from_id(id) else {
            return;
        };
        let Some(name) =
            crate::output::settings_for(kind, &self.app.settings).map(|o| o.name.clone())
        else {
            return;
        };
        if let Some((key, detail)) = self.app.spout.frame(kind, &name, frame, context.gpu()) {
            crate::debug_log(&format!("spout {kind:?}: {detail}"));
            let locale = self.app.ui.locale.get_untracked();
            let state = crate::ui::link_text::spout_send(locale, key);
            if self.app.spout.status.get(&kind) != Some(&state) {
                self.app.spout.status.insert(kind, state);
                self.app.refresh_spout_status();
                context.dispatch(Msg::UiChanged);
            }
        }
    }

    fn window_output_status(
        &mut self,
        id: WindowId,
        status: nana_ui::WindowOutputStatus,
        context: &RuntimeProgramContext<Msg>,
    ) {
        if let Some(kind) = WindowKind::from_id(id) {
            crate::debug_log(&format!("output {kind:?}: {status:?}"));
            let locale = self.app.ui.locale.get_untracked();
            let previous = self.app.ui.status.get_untracked();
            self.app.spout.status_changed(kind, &status, locale);
            self.app.refresh_spout_status();
            if !Arc::ptr_eq(&previous, &self.app.ui.status.get_untracked()) {
                context.dispatch(Msg::UiChanged);
            }
        }
    }

    fn frame_demand(&self, id: WindowId) -> FrameDemand {
        let now = clock::now_ms();
        let ui = self.app.ui;
        let animating = match WindowKind::from_id(id) {
            Some(WindowKind::Display) => {
                dialogue::display_animating(ui, now)
                    || crate::ui::sweep::animating(ui)
                    || self.follow.wants_frame()
                    || self.pager.wants_frame()
            }
            Some(WindowKind::PeerDisplay) => {
                ui.peer.get_untracked().active
                    && (dialogue::display_animating(ui.remote(), now)
                        || crate::ui::sweep::animating(ui.remote())
                        || self.peer_follow.wants_frame())
            }
            Some(WindowKind::History) => history_fading(ui, now),
            Some(WindowKind::Input) => input::mirror_chip_pulsing(ui),
            Some(WindowKind::Settings) => crate::ui::settings::links_pulsing(ui),
            _ => false,
        };
        if animating {
            // The pulses are slow; 30 fps is smooth enough for them.
            let fps = if matches!(
                WindowKind::from_id(id),
                Some(WindowKind::Input | WindowKind::Settings)
            ) {
                30
            } else {
                60
            };
            FrameDemand::Continuous(NonZeroU32::new(fps).expect("non-zero"))
        } else if let Some(at) = match WindowKind::from_id(id) {
            Some(WindowKind::History) => next_history_fade(ui, now),
            Some(WindowKind::Display) => self.follow.next_line_at(),
            Some(WindowKind::PeerDisplay) if ui.peer.get_untracked().active => {
                self.peer_follow.next_line_at()
            }
            _ => None,
        } {
            FrameDemand::At(clock::instant(at))
        } else {
            FrameDemand::OnDemand
        }
    }

    fn prepare_window_frame(&mut self, id: WindowId, _: &RuntimeProgramContext<Msg>) {
        if let Some(perf) = &mut self.perf {
            perf.begin(id);
        }
        let now = clock::now_ms();
        match WindowKind::from_id(id) {
            Some(WindowKind::Display) => self.follow_track(now, false),
            Some(WindowKind::PeerDisplay) if self.app.peer.handle.is_some() => {
                self.follow_track(now, true)
            }
            _ => {}
        }
        // The animation clock only runs for what animates on it; following
        // the text drives its own scroll.
        let ui = self.app.ui;
        let (clock, animating) = match WindowKind::from_id(id) {
            Some(WindowKind::Display) => (ui.now, dialogue::display_animating(ui, now)),
            Some(WindowKind::PeerDisplay) => (
                ui.peer_now,
                ui.peer.get_untracked().active && dialogue::display_animating(ui.remote(), now),
            ),
            Some(WindowKind::History) => (ui.history_now, history_fading(ui, now)),
            Some(WindowKind::Input) => (ui.pulse_ms, input::mirror_chip_pulsing(ui)),
            Some(WindowKind::Settings) => {
                (ui.settings_pulse_ms, crate::ui::settings::links_pulsing(ui))
            }
            _ => return,
        };
        let changed = self.docs.get(&id).is_some_and(|doc| {
            doc.context().has_pending_reactive() || doc.context().world().has_pending_work()
        });
        if animating || changed {
            clock.set_if_changed(now);
        }
    }
}

/// Read the same pending work the runtime will drain, including direct editor
/// commands and toasts. This avoids a second, incomplete message-to-window map.
fn pending_redraw(docs: &HashMap<WindowId, RuntimeDocument>) -> nana_ui::RuntimeRedraw {
    nana_ui::RuntimeRedraw::for_windows(docs.iter().filter_map(|(id, doc)| {
        let cx = doc.context();
        (cx.has_pending_reactive() || cx.world().has_pending_work()).then_some(*id)
    }))
}

#[cfg(test)]
mod redraw_tests {
    use super::*;
    use nana_ui::runtime::view::{signal, widget};
    use nana_ui::runtime::{LayoutViewport, Text};

    #[test]
    fn local_signal_changes_redraw_one_window_global_changes_redraw_both() {
        let local = signal(0);
        let global = signal(0);
        let mut docs = HashMap::new();
        let mut shaper = nana_ui::NanaTextShaper::default();
        for n in 1..=2 {
            let id = DocumentId::new(n).unwrap();
            let mut doc = RuntimeDocument::new(id);
            doc.context_mut()
                .mount_view_root(id, move || {
                    widget(Text::new("")).bind(move |t: &mut Text| {
                        t.value =
                            format!("{} {}", if n == 1 { local.get() } else { 0 }, global.get());
                    })
                })
                .unwrap();
            doc.flush(LayoutViewport::new(200.0, 100.0), &mut shaper)
                .unwrap();
            docs.insert(WindowId(n), doc);
        }
        assert_eq!(pending_redraw(&docs), nana_ui::RuntimeRedraw::None);
        local.set(1);
        assert_eq!(
            pending_redraw(&docs),
            nana_ui::RuntimeRedraw::Window(WindowId(1))
        );
        for doc in docs.values_mut() {
            doc.flush(LayoutViewport::new(200.0, 100.0), &mut shaper)
                .unwrap();
        }
        global.set(1);
        assert_eq!(
            pending_redraw(&docs),
            nana_ui::RuntimeRedraw::Windows(vec![WindowId(1), WindowId(2)])
        );
    }
}
