//! Application state and the single update function.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use nana_ui::{CompiledTheme, RuntimeProgramContext, RuntimeProgramUpdate};
use nana_ui_core::{FileDialogKind, FileDialogRequest, FileFilter};
use nana_ui_platform::host::WindowCommand;
use nanatype_audio::{AudioEngine, AudioHandle, AudioStatus, Bus, Scheduled};
use nanatype_blive::{BliveConfig, BliveHandle, BliveStatus, RoomKey};
use nanatype_bridge::{
    BridgeEvent, BridgeHandle, BridgeStatus, LipSync, OscConfig, Sink, VtsConfig, WsConfig,
};
use nanatype_core::{Cmd, Core, Effects, Event, UiSound};
use nanatype_i18n::Locale;
use nanatype_markup::{parse, serialize};
use nanatype_model::{AssetRef, Document, Inline};
use nanatype_playback::CueKind;
use nanatype_settings::{GeometryStore, InputPresence, LoadNote, SendKey, Settings};
use nanatype_theme::{LoadedSkin, SkinRegistry, TomlTable};

use crate::assets::AssetLibrary;
use crate::chat::ChatMessage;
use crate::clock;
use crate::msg::{FileTarget, GlobalAction, MarkupEdit, Msg};
use crate::paths::Paths;
use crate::scheduler::Scheduler;
use crate::services::chat_assets::ChatAssets;
use crate::services::hotkeys::{self, Hotkeys};
use crate::services::persist::Persist;
use crate::services::sounds::{self, Bank};
use crate::services::tray::Tray;
use crate::skin_theme;
use crate::ui::feedback::{self, ToastRequest};
use crate::ui::{I18N, LinkState, Notice, StickerRow, Ui};
use crate::windows::{self, WindowKind, WindowManager};

/// Chat messages the panel keeps.
const CHAT_CAP: usize = 500;

/// Send-key mode shared with the editor's key handler.
pub static SEND_KEY: AtomicU8 = AtomicU8::new(0);

pub struct App {
    pub paths: Paths,
    pub settings: Settings,
    pub readonly_settings: bool,
    pub registry: SkinRegistry,
    pub loaded: LoadedSkin,
    pub skin: Arc<nanatype_theme::Skin>,
    pub core: Core,
    pub ui: Ui,
    pub assets: AssetLibrary,
    pub windows: WindowManager,
    pub scheduler: Scheduler,
    pub audio: AudioHandle,
    pub bank: Bank,
    pub bridge: BridgeHandle,
    /// The blivechat connection and the thread saving its pictures.
    pub blive: BliveHandle,
    _chat_assets: ChatAssets,
    /// Live chat, oldest first, capped at the panel's size.
    chat: VecDeque<Arc<ChatMessage>>,
    pub hotkeys: Hotkeys,
    pub tray: Option<Tray>,
    pub persist: Persist,
    pub theme: Arc<CompiledTheme>,
    pub ctx: RuntimeProgramContext<Msg>,
    /// The input window was summoned (summon presence).
    pub summoned: bool,
    pub system_dark: bool,
    next_notice: u64,
    dialogs: HashMap<u64, FileTarget>,
    next_dialog: u64,
    /// Pending window commands for the next update result.
    pub commands: Vec<WindowCommand>,
    pub theme_changed: bool,
    pub quitting: bool,
    export_target: Option<String>,
    pub spout: crate::output::Spout,
    /// Editor commands for the host to apply to the input window's editor.
    pub rich_commands: Vec<nana_ui::runtime::RichEditCommand>,
    /// Fonts packaged with the active skin, registered while it is active.
    fonts: nana_ui::HostFontScope,
    /// Family registered for each skin font role (body, name, accent).
    skin_families: [Option<String>; 3],
    skin_watch: Option<crate::services::watch::Watch>,
    sticker_watch: Option<crate::services::watch::Watch>,
    /// Toasts for the host to show in the settings and input windows.
    pub toasts: Vec<ToastRequest>,
    /// The latest notice, while it is an error: the tray tooltip shows it.
    tray_error: Option<(u64, String)>,
}

/// How long an error stays in the tray tooltip.
const TRAY_ERROR_SECS: u64 = 30;

fn locale_of(s: &Settings) -> Locale {
    s.general
        .locale
        .as_deref()
        .and_then(Locale::from_tag)
        .unwrap_or_else(Locale::system)
}

impl App {
    pub fn new(ctx: &RuntimeProgramContext<Msg>) -> Self {
        let paths = Paths::resolve();
        paths.ensure();
        // Before any window draws a picture: NanaUI reads local pictures
        // only under this base, and the rest are mirrored into it.
        crate::services::media::init(&paths.data);
        let legacy = Paths::legacy_settings();
        let loaded_settings =
            nanatype_settings::load_settings(&paths.settings(), legacy.as_deref());
        let mut settings = loaded_settings.value;
        settings.sanitize();
        let geometry: GeometryStore = nanatype_settings::load_json(&paths.geometry()).value;

        let post = {
            let ctx = ctx.clone();
            move |m: Msg| ctx.dispatch(m)
        };
        let persist = Persist::start({
            let post = post.clone();
            move |e| {
                post(Msg::Notice {
                    text: e,
                    error: true,
                })
            }
        });
        let registry = SkinRegistry::new(Some(paths.skins()), env!("CARGO_PKG_VERSION"));
        let (loaded, fallback) = registry.load_or_default(&settings.skin.id);
        let skin = Arc::new(skin_theme::with_overrides(
            &loaded.skin,
            &settings.skin.overrides,
        ));
        let locale = locale_of(&settings);
        let system_dark = matches!(
            ctx.system_appearance(),
            Some(nana_ui_platform::SystemAppearance::Dark)
        );
        let theme = skin_theme::compile(
            &skin,
            skin_theme::dark_mode(&skin, settings.skin.chrome, system_dark),
        );
        let ui = Ui::new(Arc::new(settings.clone()), skin.clone(), locale);
        ui.system_reduced_motion.set(ctx.reduced_motion());
        ui.reduced_motion
            .set(settings.playback.reduced_motion || ctx.reduced_motion());

        let scheduler = Scheduler::start(post.clone());
        let audio = AudioEngine::start(settings.sound.device.clone(), {
            let post = post.clone();
            move |s| post(Msg::Audio(s))
        });
        let bridge = BridgeHandle::new({
            let post = post.clone();
            move |s| post(Msg::Bridge(s))
        });
        let chat_assets = ChatAssets::start(paths.blobs(), {
            let post = post.clone();
            move |m| post(Msg::Chat(Arc::new(m)))
        });
        let blive = {
            let to_assets = chat_assets.sender();
            let post = post.clone();
            BliveHandle::new(
                move |s| post(Msg::ChatStatus(s)),
                move |e| {
                    let _ = to_assets.send(e);
                },
            )
        };
        let hotkeys = Hotkeys::start(
            {
                let post = post.clone();
                move |a| post(Msg::Hotkey(a))
            },
            {
                let post = post.clone();
                move |s| post(Msg::HotkeyStatus(s))
            },
        );
        let core = Core::new(settings.clone(), skin.clone());
        let mut app = Self {
            assets: AssetLibrary::new(paths.blobs()),
            paths,
            readonly_settings: matches!(loaded_settings.note, LoadNote::TooNew { .. }),
            registry,
            loaded,
            skin,
            core,
            ui,
            windows: {
                let mut w = WindowManager::default();
                w.geometry = geometry;
                w
            },
            scheduler,
            audio,
            bank: Vec::new(),
            bridge,
            blive,
            _chat_assets: chat_assets,
            chat: VecDeque::new(),
            hotkeys,
            tray: None,
            persist,
            theme,
            ctx: ctx.clone(),
            summoned: false,
            system_dark,
            next_notice: 1,
            dialogs: HashMap::new(),
            next_dialog: 1,
            commands: Vec::new(),
            theme_changed: false,
            quitting: false,
            export_target: None,
            spout: crate::output::Spout::default(),
            rich_commands: Vec::new(),
            fonts: nana_ui::HostFontScope::new(),
            skin_families: [None, None, None],
            skin_watch: None,
            sticker_watch: None,
            toasts: Vec::new(),
            tray_error: None,
            settings,
        };
        match loaded_settings.note {
            LoadNote::Quarantined { moved_to, .. } => {
                let text = app.ui.tr(
                    "err-settings-quarantined",
                    &[("path", moved_to.display().to_string().into())],
                );
                app.notice(text, true);
            }
            LoadNote::TooNew { .. } => {
                let text = app.ui.now_t("err-settings-newer");
                app.notice(text, true);
            }
            LoadNote::Migrated { .. } => app.save_settings(),
            _ => {}
        }
        if let Some(_errors) = fallback {
            let text = I18N.tr_in(
                locale,
                "skins-fallback",
                &[("id", app.settings.skin.id.as_str().into())],
            );
            app.notice(text, true);
        }
        match Tray::build(post) {
            Ok(tray) => app.tray = Some(tray),
            Err(e) => app.notice(format!("tray: {e}"), true),
        }
        app.register_fonts();
        app.apply_skin();
        app.refresh_skin_rows();
        app.refresh_assets();
        app.load_sounds();
        app.apply_services();
        app.audio_levels();
        if let Some(tray) = &mut app.tray {
            tray.relabel(&I18N, locale);
            tray.sync(&app.settings);
        }
        app.sync_tray_skins();
        app.sync_tray();
        SEND_KEY.store(
            matches!(app.settings.general.send_key, SendKey::CtrlEnter) as u8,
            Ordering::Relaxed,
        );
        app.request_displays();
        if !app.settings.general.onboarding_done {
            app.open_settings();
        }
        if let Some(ms) = std::env::var("NANATYPE_EXIT_AFTER")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(ms));
                ctx.dispatch(Msg::Quit);
            });
        }
        if cfg!(debug_assertions) && std::env::var_os("NANATYPE_DEMO").is_some() {
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                ctx.dispatch(Msg::PlaySample);
                if std::env::var("NANATYPE_DEMO").as_deref() == Ok("editor") {
                    demo_editor(&ctx);
                }
            });
        }
        let ctx = ctx.clone();
        crate::listen_for_instances(move || ctx.dispatch(Msg::SecondInstance));
        app
    }

    pub fn post(&self, msg: Msg) {
        self.ctx.dispatch(msg);
    }

    pub fn notice(&mut self, text: String, error: bool) {
        let id = self.next_notice;
        self.next_notice += 1;
        let mut status = (*self.ui.status.get_untracked()).clone();
        status.notices.push(Notice {
            id,
            text: text.clone(),
            error,
        });
        if status.notices.len() > 4 {
            status.notices.remove(0);
        }
        self.ui.status.set(Arc::new(status));
        let tone = if error {
            nana_ui::ToastTone::Danger
        } else {
            nana_ui::ToastTone::Info
        };
        self.toasts.push(ToastRequest::new(
            text.clone(),
            tone,
            feedback::NOTICE_MS,
            true,
        ));
        // The tray says the latest error, so it is seen with every window
        // hidden (summon mode) too.
        self.tray_error = error.then(|| (id, text));
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(feedback::NOTICE_MS));
            ctx.dispatch(Msg::DismissNotice(id));
            if error {
                std::thread::sleep(
                    Duration::from_secs(TRAY_ERROR_SECS)
                        .saturating_sub(Duration::from_millis(feedback::NOTICE_MS)),
                );
                ctx.dispatch(Msg::TrayErrorExpired(id));
            }
        });
    }

    /// The input window is on screen (or about to be).
    pub fn input_visible(&self) -> bool {
        self.windows.is_open(WindowKind::Input)
            && windows::desired(WindowKind::Input, &self.skin, &self.settings, self.summoned)
                .visible
    }

    /// Tray labels and tooltip for the current state.
    fn sync_tray(&self) {
        let Some(tray) = &self.tray else {
            return;
        };
        let s = &self.settings;
        let locale = self.ui.locale.get_untracked();
        let skin = self
            .ui
            .skins
            .with_untracked(|rows| {
                rows.iter()
                    .find(|r| r.id == s.skin.id)
                    .map(|r| r.name.clone())
            })
            .unwrap_or_else(|| s.skin.id.clone());
        let locked = s.overlays.display.lock == nanatype_settings::Lock::Locked;
        let tooltip = crate::services::tray::tooltip(
            &I18N,
            locale,
            s.general.input_mode,
            &skin,
            locked,
            self.tray_error.as_ref().map(|(_, e)| e.as_str()),
        );
        let meta = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Win"
        };
        tray.show(
            &I18N,
            crate::services::tray::TrayState {
                locale,
                input: windows::desired(WindowKind::Input, &self.skin, s, self.summoned).visible,
                display: s.overlays.display.visible,
                history: s.overlays.history.visible,
                chat: s.integrations.blivechat.panel_open,
                locked,
                continue_key: GlobalAction::Continue.get(s).map(|h| h.label(meta)),
                tooltip,
                badge: crate::services::tray::Badge {
                    mirroring: s.general.input_mode == nanatype_settings::InputMode::Mirror,
                    hidden: !s.overlays.display.visible && !s.overlays.history.visible,
                    locked,
                },
            },
        );
    }

    /// Tint the system title bars (settings and the chat panel) with the
    /// chrome.
    pub fn tint_captions(&self) {
        for kind in [WindowKind::Settings, WindowKind::Chat] {
            if let Some(handle) = self.windows.handle(kind) {
                windows::tint_caption(handle, &self.theme);
            }
        }
    }

    fn edit_status(&self, f: impl FnOnce(&mut crate::ui::Status)) {
        let mut status = (*self.ui.status.get_untracked()).clone();
        f(&mut status);
        self.ui.status.set(Arc::new(status));
    }

    fn request_displays(&self) {
        let service = self.ctx.windows().clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let Ok(displays) = service.displays().wait() else {
                return;
            };
            let scale = displays
                .iter()
                .find(|d| d.primary)
                .or(displays.first())
                .map(|d| d.scale_factor)
                .unwrap_or(1.0);
            let monitors = displays
                .iter()
                .filter_map(|d| {
                    let b = d.logical_bounds(scale)?;
                    let (w, h) = d.physical_size.unwrap_or((0, 0));
                    Some(nanatype_settings::MonitorInfo {
                        key: nanatype_settings::MonitorKey(format!(
                            "{}|{w}x{h}",
                            d.name.clone().unwrap_or_else(|| format!("{:x}", d.id.0))
                        )),
                        work_area: [
                            b.position.0,
                            b.position.1,
                            b.size.0,
                            b.size.1 - 48.0 / scale,
                        ],
                        primary: d.primary,
                    })
                })
                .collect();
            ctx.dispatch(Msg::Displays(monitors));
        });
    }

    // ---- skins and assets ------------------------------------------------

    fn refresh_skin_rows(&mut self) {
        let locale = self.ui.locale.get_untracked();
        let rows = crate::ui::skin_rows(&self.registry, locale);
        self.ui.skins.set(Arc::new(rows));
    }

    fn sync_tray_skins(&mut self) {
        let locale = self.ui.locale.get_untracked();
        let skins: Vec<(String, String)> = self
            .ui
            .skins
            .get_untracked()
            .iter()
            .filter(|r| r.error.is_none())
            .map(|r| (r.id.clone(), r.name.clone()))
            .collect();
        let _ = locale;
        if let Some(tray) = &mut self.tray {
            tray.set_skins(&skins, &self.settings.skin.id);
        }
    }

    /// Asset URLs for skin files and stickers, for the views.
    fn refresh_assets(&mut self) {
        self.assets.scan(&self.settings.assets.sticker_dirs);
        let mut urls = crate::assets::skin_urls(&*self.loaded.files);
        let stickers: Vec<StickerRow> = self
            .assets
            .stickers
            .iter()
            .filter_map(|s| {
                // A sticker NanaUI cannot load (too large, a link out of
                // its folder) is left out of the panel.
                let url = s.url()?;
                urls.insert(s.asset().to_string(), url.clone());
                Some(StickerRow {
                    asset: s.asset().to_string(),
                    name: s.name.clone(),
                    lib: s.lib.clone(),
                    url,
                })
            })
            .collect();
        // Pictures of files, pasted images and chat in the current views.
        let mut wanted = doc_assets(self.core.draft());
        wanted.extend(display_assets(self.core.display()));
        for row in self.core.history() {
            wanted.extend(row_assets(&row.item, &row.doc));
        }
        for m in &self.chat {
            wanted.extend(message_assets(m));
        }
        for asset in wanted {
            let key = asset.to_string();
            if !urls.contains_key(&key)
                && let Some(u) = self.assets.url(&asset, Some(&self.loaded))
            {
                urls.insert(key, u);
            }
        }
        let mut portraits = HashMap::new();
        let p = &self.skin.dialogue.portrait;
        if let Some(root) = self.loaded.files.root() {
            if let Some(url) = p
                .image
                .as_deref()
                .and_then(|img| crate::assets::skin_file_url(root, img))
            {
                portraits.insert("default".to_owned(), url);
            }
            for (name, img) in &p.variants {
                if let Some(url) = crate::assets::skin_file_url(root, img) {
                    portraits.insert(name.clone(), url);
                }
            }
        }
        self.ui.asset_urls.set(Arc::new(urls));
        self.ui.stickers.set(Arc::new(stickers));
        self.ui.portraits.set(Arc::new(portraits));
    }

    fn load_sounds(&self) {
        let skin = self.loaded.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let (bank, errors) = sounds::load(&skin);
            ctx.dispatch(Msg::SoundsLoaded {
                skin: skin.skin.meta.id.clone(),
                bank,
                errors,
            });
        });
    }

    fn reload_skin(&mut self) {
        let (loaded, fallback) = self.registry.load_or_default(&self.settings.skin.id);
        if let Some(errors) = fallback {
            let detail = errors.first().map(|d| d.to_string()).unwrap_or_default();
            let text = self.ui.tr(
                "skins-fallback",
                &[("id", self.settings.skin.id.as_str().into())],
            );
            self.notice(format!("{text}: {detail}"), true);
        }
        let changed_files = loaded.skin.meta.id != self.loaded.skin.meta.id
            || loaded.skin.sounds != self.loaded.skin.sounds;
        self.loaded = loaded;
        self.register_fonts();
        self.apply_skin();
        if changed_files {
            self.load_sounds();
        }
        self.refresh_assets();
    }

    /// Register the skin's packaged fonts, replacing the previous skin's.
    fn register_fonts(&mut self) {
        self.fonts.clear();
        let f = self.loaded.skin.fonts.clone();
        let files = self.loaded.files.clone();
        let mut families = [None, None, None];
        let mut errors = Vec::new();
        for (i, spec) in [&f.body, &f.name, &f.accent].into_iter().enumerate() {
            let Some(file) = &spec.file else {
                continue;
            };
            match files.read(file) {
                Ok(bytes) => match self.fonts.add_bytes(bytes) {
                    Ok(registration) => families[i] = registration.families().first().cloned(),
                    Err(e) => errors.push(format!("{file}: {e}")),
                },
                Err(e) => errors.push(e.to_string()),
            }
        }
        self.skin_families = families;
        for e in errors {
            self.notice(e, true);
        }
    }

    fn apply_skin(&mut self) {
        let skin = Arc::new(skin_theme::prepare(
            &self.loaded.skin,
            &self.settings.skin.overrides,
            &self.skin_families,
            self.ui.locale.get_untracked().tag(),
        ));
        self.skin = skin.clone();
        let dark = skin_theme::dark_mode(&skin, self.settings.skin.chrome, self.system_dark);
        self.theme = skin_theme::compile(&skin, dark);
        self.theme_changed = true;
        self.tint_captions();
        self.ui.skin.set(skin.clone());
        let fx = self.core.handle(Cmd::SetSkin(skin), clock::now_ms());
        self.apply_effects(fx);
    }

    // ---- services ---------------------------------------------------------

    fn hotkey_bindings(&self) -> hotkeys::Bindings {
        GlobalAction::all_in(&self.settings)
            .filter_map(|a| a.get(&self.settings).cloned().map(|h| (a, h)))
            .collect()
    }

    fn apply_services(&mut self) {
        self.hotkeys.apply(
            self.hotkey_bindings(),
            self.ui.recording.get_untracked().is_some(),
        );
        let i = &self.settings.integrations;
        self.bridge.set_vts(i.vts.enabled.then(|| VtsConfig {
            port: i.vts.port,
            token: i.vts.token.clone(),
            expressions: i.vts.expressions.clone(),
            lip_sync: (i.vts.lip_sync && !i.vts.mouth_param.trim().is_empty()).then(|| LipSync {
                param: i.vts.mouth_param.trim().to_owned(),
                amp: i.vts.mouth_amp,
            }),
            reset: i.vts.reset_expressions,
        }));
        self.bridge.set_osc(i.osc.enabled.then(|| OscConfig {
            host: i.osc.host.clone(),
            port: i.osc.port,
            prefix: i.osc.prefix.clone(),
        }));
        self.bridge.set_ws(i.websocket.enabled.then_some(WsConfig {
            port: i.websocket.port,
        }));
        let b = &i.blivechat;
        let room = RoomKey::parse(
            b.room_kind == nanatype_settings::BliveRoomKind::AuthCode,
            &b.room,
        );
        let blive = room.filter(|_| b.enabled).map(|room| BliveConfig {
            host: b.host.clone(),
            port: b.port,
            room,
        });
        let blive_off = blive.is_none();
        let blive_hint = self.ui.now_t(if b.enabled {
            "chat-status-no-room"
        } else {
            "int-status-off"
        });
        self.blive.set(blive);
        self.edit_status(|s| {
            if !i.vts.enabled {
                s.vts = LinkState::Off;
            }
            if !i.osc.enabled {
                s.osc = LinkState::Off;
            }
            if !i.websocket.enabled {
                s.ws = LinkState::Off;
            }
            if blive_off {
                s.blive = blive_hint;
                s.blive_link = crate::ui::danmaku::Link::Off;
            }
        });
        self.refresh_spout_status();
        self.refresh_watches();
    }

    /// Keep file watchers on the folders that matter right now.
    fn refresh_watches(&mut self) {
        let post = {
            let ctx = self.ctx.clone();
            move |m: Msg| ctx.dispatch(m)
        };
        let skin_dir = self
            .loaded
            .files
            .root()
            .filter(|_| self.settings.skin.hot_reload)
            .map(|p| vec![p.to_path_buf()]);
        let current = self.skin_watch.as_ref().map(|w| w.paths().to_vec());
        if skin_dir != current {
            self.skin_watch = skin_dir.map(|paths| {
                crate::services::watch::Watch::start(paths, post.clone(), || Msg::SkinFilesChanged)
            });
        }
        let stickers: Vec<PathBuf> = self
            .settings
            .assets
            .sticker_dirs
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .collect();
        let current = self
            .sticker_watch
            .as_ref()
            .map(|w| w.paths().to_vec())
            .unwrap_or_default();
        if stickers != current {
            self.sticker_watch = (!stickers.is_empty()).then(|| {
                crate::services::watch::Watch::start(stickers, post, || Msg::AssetsChanged)
            });
        }
    }

    pub fn refresh_spout_status(&mut self) {
        let locale = self.ui.locale.get_untracked();
        self.spout.sync(&self.settings, locale);
        let display = self
            .spout
            .status
            .get(&WindowKind::Display)
            .cloned()
            .unwrap_or_default();
        let history = self
            .spout
            .status
            .get(&WindowKind::History)
            .cloned()
            .unwrap_or_default();
        self.edit_status(|s| {
            s.spout_display = display;
            s.spout_history = history;
        });
    }

    fn audio_levels(&self) {
        let s = &self.settings.sound;
        self.audio.set_master(s.master);
        self.audio.set_bus(Bus::Typing, s.typing_volume);
        self.audio.set_bus(Bus::Effects, s.effects_volume);
        self.audio.set_bus(Bus::Ui, s.ui_volume);
    }

    fn save_settings(&self) {
        if !self.readonly_settings {
            self.persist.json(self.paths.settings(), &self.settings);
        }
    }

    pub fn save_geometry(&mut self) {
        if self.windows.geometry_dirty {
            self.windows.geometry_dirty = false;
            self.persist
                .json(self.paths.geometry(), &self.windows.geometry);
        }
    }

    fn apply_settings(&mut self, next: Settings) {
        if next == self.settings {
            return;
        }
        let prev = std::mem::replace(&mut self.settings, next);
        let s = &self.settings;
        let locale_changed = locale_of(&prev) != locale_of(s);
        let skin_changed = prev.skin.id != s.skin.id;
        let look_changed =
            prev.skin.overrides != s.skin.overrides || prev.skin.chrome != s.skin.chrome;
        let assets_changed = prev.assets.sticker_dirs != s.assets.sticker_dirs;
        let device_changed = prev.sound.device != s.sound.device;
        let autostart_changed = prev.general.launch_at_login != s.general.launch_at_login;
        SEND_KEY.store(
            matches!(s.general.send_key, SendKey::CtrlEnter) as u8,
            Ordering::Relaxed,
        );
        self.sync_reduced_motion();
        if locale_changed {
            let locale = locale_of(s);
            self.ui.locale.set(locale);
            if let Some(tray) = &self.tray {
                tray.relabel(&I18N, locale);
            }
            self.refresh_skin_rows();
            // The font stack follows the language.
            self.apply_skin();
            self.sync_tray_skins();
            for kind in [
                WindowKind::Display,
                WindowKind::History,
                WindowKind::Settings,
                WindowKind::Chat,
            ] {
                self.commands.push(WindowCommand::SetTitle {
                    id: kind.id(),
                    title: I18N.t_in(locale, kind.title_key()),
                });
            }
        }
        if skin_changed {
            self.reload_skin();
            self.sync_tray_skins();
        } else if look_changed {
            self.apply_skin();
        }
        if assets_changed {
            self.refresh_assets();
        }
        if device_changed {
            self.audio.set_device(self.settings.sound.device.clone());
        }
        if autostart_changed
            && let Err(e) = crate::services::set_autostart(self.settings.general.launch_at_login)
        {
            self.notice(e, true);
        }
        self.audio_levels();
        self.apply_services();
        let fx = self.core.handle(
            Cmd::SetSettings(Box::new(self.settings.clone())),
            clock::now_ms(),
        );
        self.apply_effects(fx);
        self.ui.settings.set(Arc::new(self.settings.clone()));
        if let Some(tray) = &self.tray {
            tray.sync(&self.settings);
        }
        self.save_settings();
    }

    pub fn edit(&mut self, f: impl FnOnce(&mut Settings)) {
        let mut next = self.settings.clone();
        f(&mut next);
        next.sanitize();
        self.apply_settings(next);
    }

    // ---- the session ------------------------------------------------------

    pub fn run(&mut self, cmd: Cmd) {
        let fx = self.core.handle(cmd, clock::now_ms());
        self.apply_effects(fx);
    }

    fn play_cue_now(&mut self, cue: &CueKind) {
        if let Some(v) = sounds::voice(cue, &self.loaded) {
            self.audio.play(v);
        } else {
            self.run(Cmd::CueFired(cue.clone()));
        }
    }

    fn apply_effects(&mut self, fx: Effects) {
        for epoch in &fx.cancel_epochs {
            self.audio.cancel(*epoch);
            self.scheduler.cancel(*epoch);
        }
        for wake in &fx.wakes {
            self.scheduler.wake(*wake);
        }
        for batch in &fx.cues {
            let mut audio = Vec::new();
            for cue in batch.cues.iter() {
                let at = batch.origin_ms + u64::from(cue.at_ms);
                match sounds::voice(&cue.kind, &self.loaded) {
                    Some(voice) => audio.push(Scheduled {
                        at: clock::instant(at),
                        voice,
                    }),
                    None => self.scheduler.cue(batch.epoch, at, cue.kind.clone()),
                }
            }
            self.audio.schedule(batch.epoch, audio);
        }
        for cue in &fx.fire_now {
            self.play_cue_now(&cue.kind);
        }
        for sound in &fx.ui_sounds {
            let id = match sound {
                UiSound::Open => sounds::OPEN,
                UiSound::Advance => sounds::ADVANCE,
            };
            self.audio.play(nanatype_audio::Voice {
                sound: id.into(),
                bus: Bus::Ui,
                gain: 1.0,
                pitch: 1.0,
            });
        }
        for event in &fx.events {
            self.bridge_event(event);
        }
        if let Some(doc) = &fx.replace_draft {
            self.set_source(serialize(doc), true);
        }
        if fx.hide_input {
            self.summoned = false;
        }
        if fx.display_changed {
            self.ui.display.set(Arc::new(self.core.display().clone()));
            self.ui.now.set(clock::now_ms());
        }
        if fx.history_changed {
            self.ui
                .history
                .set(Arc::new(self.core.history().iter().cloned().collect()));
        }
        let queued = self.core.queued();
        if self.ui.status.with_untracked(|s| s.queued != queued) {
            self.edit_status(|s| s.queued = queued);
        }
        if fx.reply_changed {
            self.ui.reply.set(self.core.pending_reply().cloned());
        }
        if fx.display_changed || fx.history_changed {
            self.refresh_atom_urls();
        }
    }

    /// Make sure every atom on screen has a URL.
    /// Resolve the pictures of the display, the draft and the newest
    /// history row.
    fn refresh_atom_urls(&mut self) {
        let mut assets = display_assets(self.core.display());
        assets.extend(doc_assets(self.core.draft()));
        if let Some(row) = self.core.history().back() {
            assets.extend(row_assets(&row.item, &row.doc));
        }
        self.ensure_urls(assets);
    }

    /// Give every asset of `assets` a URL the views can load.
    fn ensure_urls(&mut self, assets: impl IntoIterator<Item = AssetRef>) {
        let urls = self.ui.asset_urls.get_untracked();
        let found: Vec<(String, String)> = assets
            .into_iter()
            .filter(|asset| !urls.contains_key(&asset.to_string()))
            .filter_map(|asset| {
                Some((
                    asset.to_string(),
                    self.assets.url(&asset, Some(&self.loaded))?,
                ))
            })
            .collect();
        if found.is_empty() {
            return;
        }
        let mut next = (*urls).clone();
        next.extend(found);
        self.ui.asset_urls.set(Arc::new(next));
    }

    fn bridge_event(&mut self, event: &Event) {
        let out = match event {
            Event::LineStarted(line) => BridgeEvent::LineStart {
                id: line.id.0,
                text: line.doc.plain_text(),
                markup: serialize(&line.doc),
            },
            Event::LineFinished(id) => BridgeEvent::LineEnd { id: id.0 },
            Event::Expression { name, intensity } => BridgeEvent::Expression {
                name: name.clone(),
                intensity: *intensity,
            },
            Event::Portrait(v) => BridgeEvent::Portrait { variant: v.clone() },
            Event::Speaking(s) => BridgeEvent::Speaking { speaking: *s },
            Event::Shake { amp, ms } => BridgeEvent::Shake { amp: *amp, ms: *ms },
            Event::Vts(action) => BridgeEvent::Vts {
                action: action.clone(),
            },
            Event::LineRevealed(_) | Event::Queued { .. } => return,
        };
        self.bridge.emit(out);
    }

    // ---- the editor ---------------------------------------------------------

    /// Replace the editor text. `program` marks it as not typed by the user.
    fn set_source(&mut self, text: String, program: bool) {
        let mut src = self.ui.source.get_untracked();
        if src.text == text && !program {
            return;
        }
        let len = text.len();
        src.text = text;
        if program {
            src.revision += 1;
            src.selection = (len, len);
        }
        self.ui.source.set(src.clone());
        self.reparse(&src.text);
    }

    fn reparse(&mut self, text: &str) {
        let parsed = parse(text);
        let locale = self.ui.locale.get_untracked();
        let diags: Vec<String> = parsed
            .diagnostics
            .iter()
            .filter(|d| !matches!(d.kind, nanatype_markup::DiagnosticKind::Unclosed(_)))
            .map(|d| {
                let pos = text[..d.range.start.min(text.len())].chars().count() + 1;
                let message = match &d.kind {
                    nanatype_markup::DiagnosticKind::UnknownTag(t) => format!("[{t}] ?"),
                    nanatype_markup::DiagnosticKind::BadValue { tag, value } => {
                        format!("[{tag}={value}] ?")
                    }
                    nanatype_markup::DiagnosticKind::UnmatchedClose(t) => format!("[/{t}] ?"),
                    other => format!("{other:?}"),
                };
                I18N.tr_in(
                    locale,
                    "editor-diagnostic",
                    &[("pos", pos.into()), ("message", message.into())],
                )
            })
            .collect();
        self.ui.diagnostics.set(Arc::new(diags));
        let doc = parsed.doc;
        self.ui.editor_value.set(self.editor_rich(&doc));
        self.ui.draft.set(Arc::new(doc.clone()));
        self.run(Cmd::DraftMeta(parsed.meta));
        self.run(Cmd::DraftChanged(doc));
    }

    /// The editor's look: the dialogue text of the active skin.
    fn editor_look(&self) -> crate::ui::rich::Look {
        crate::ui::dialogue::Base::editor(&self.skin).look()
    }

    /// `doc` as the editor's rich text.
    fn editor_rich(&self, doc: &Document) -> nana_ui_core::RichText {
        let look = self.editor_look();
        let urls = self.ui.asset_urls.get_untracked();
        crate::ui::rich::to_rich(
            doc,
            &crate::ui::rich::Env {
                look: &look,
                urls: &urls,
                editor: true,
                locale: self.ui.locale.get_untracked(),
            },
        )
    }

    /// The user edited the rich text.
    fn editor_changed(&mut self, rich: nana_ui_core::RichText) {
        let doc = crate::ui::rich::from_rich(&rich, &self.editor_look());
        self.ui.editor_value.set(rich);
        let text = serialize(&doc);
        let mut src = self.ui.source.get_untracked();
        if src.text != text {
            let len = text.len();
            src.text = text;
            src.selection = (len, len);
            src.revision += 1;
            self.ui.source.set(src);
        }
        self.ui.diagnostics.set(Arc::new(Vec::new()));
        self.ui.draft.set(Arc::new(doc.clone()));
        self.run(Cmd::DraftChanged(doc));
        self.refresh_atom_urls();
    }

    /// A toolbar action: an editor command, or a markup edit in source mode.
    fn tool(&mut self, tool: crate::msg::Tool) {
        use crate::msg::Tool;
        use nana_ui::runtime::{RichAttrMask, RichEditCommand};
        use nana_ui_core::{PaintColor, RichSpanStyle, RichTextShadow, RichTextStroke};

        if self.ui.source_mode.get_untracked() {
            if let Some(edit) = markup_for(&tool) {
                self.markup(edit);
            }
            return;
        }
        let attrs = self.ui.sel_attrs.get_untracked();
        let look = self.editor_look();
        let uniform = |mask: RichAttrMask| !attrs.mixed.contains(mask);
        let paint = |c: nanatype_model::Rgba8| PaintColor::srgb(skin_theme::rgba(c));
        // Effects and pacing share one interned attribute per span.
        let (mut effects, mut pace) = match attrs.style.effect {
            Some(index) if uniform(RichAttrMask::EFFECT) => crate::ui::rich::attr_of(index),
            _ => Default::default(),
        };
        let command = match tool {
            Tool::Bold => {
                let on = uniform(RichAttrMask::WEIGHT)
                    && attrs.style.shape.weight.is_some_and(|w| w >= 600);
                RichEditCommand::SetAttrs(RichSpanStyle::new().weight(if on {
                    400.min(look.weight)
                } else {
                    700.max(look.weight + 200).min(900)
                }))
            }
            Tool::Italic => {
                let on = uniform(RichAttrMask::ITALIC) && attrs.style.shape.italic == Some(true);
                RichEditCommand::SetAttrs(RichSpanStyle::new().italic(!on))
            }
            Tool::Underline | Tool::Strike => {
                let mut d = attrs.style.paint.decoration.unwrap_or_default();
                if matches!(tool, Tool::Underline) {
                    d.underline = !(uniform(RichAttrMask::DECORATION) && d.underline);
                } else {
                    d.line_through = !(uniform(RichAttrMask::DECORATION) && d.line_through);
                }
                let mut s = RichSpanStyle::new();
                s.paint.decoration = Some(d);
                RichEditCommand::SetAttrs(s)
            }
            Tool::Color(c) => RichEditCommand::SetAttrs(RichSpanStyle::new().color(paint(c))),
            Tool::Size(m) => RichEditCommand::SetAttrs(RichSpanStyle::new().size(look.size * m)),
            Tool::Stroke(st) => {
                let (w, c) = st
                    .map(|s| (s.width_px, s.color))
                    .unwrap_or((0.0, look.color));
                RichEditCommand::SetAttrs(
                    RichSpanStyle::new().stroke(
                        RichTextStroke::new(w, paint(c))
                            .placement(nana_ui_core::TextStrokePlacement::Under),
                    ),
                )
            }
            Tool::Shadow(sh) => {
                let mut s = RichSpanStyle::new();
                s.paint.shadows = Some(match sh {
                    Some(sh) => Arc::from([RichTextShadow::new(
                        [sh.dx, sh.dy],
                        sh.blur,
                        paint(sh.color),
                    )]),
                    None => Arc::from([]),
                });
                RichEditCommand::SetAttrs(s)
            }
            Tool::Ruby(text) => {
                RichEditCommand::SetRuby((!text.trim().is_empty()).then(|| text.trim().to_owned()))
            }
            Tool::Effect(_) | Tool::Entrance(_) | Tool::Pace(_) => {
                match tool {
                    Tool::Effect(kind) => effects.toggle(kind),
                    Tool::Entrance(kind) => {
                        effects.entrance = Some(nanatype_model::Entrance {
                            kind,
                            ms: self.skin.dialogue.reveal.entrance.ms.max(120),
                        })
                    }
                    Tool::Pace(p) => pace = p,
                    _ => unreachable!(),
                }
                RichEditCommand::SetAttrs(
                    RichSpanStyle::new().effect(crate::ui::rich::attr_index_any(&effects, pace)),
                )
            }
            // Built the way the editor's text builds it: a tag for a marker,
            // a picture at its own aspect.
            Tool::Insert(inline) => {
                let doc = Document::from_inlines(vec![inline.clone()]);
                self.ensure_urls(doc_assets(&doc));
                let doc = Document::from_inlines(vec![inline]);
                match self.editor_rich(&doc).objects().first() {
                    Some((_, object)) => RichEditCommand::InsertObject(object.clone()),
                    None => return,
                }
            }
            Tool::ClearFormat => RichEditCommand::ClearFormatting,
        };
        self.rich_commands.push(command);
    }

    fn markup(&mut self, edit: MarkupEdit) {
        let mut src = self.ui.source.get_untracked();
        match edit {
            MarkupEdit::Typed { text, selection } => {
                src.selection = selection;
                let changed = src.text != text;
                src.text = text;
                self.ui.source.set(src.clone());
                if changed {
                    self.reparse(&src.text);
                }
            }
            MarkupEdit::Wrap { open, close } => {
                let (a, b) = clamp_sel(&src.text, src.selection);
                let mut text = src.text.clone();
                text.insert_str(b, &close);
                text.insert_str(a, &open);
                let caret = if a == b {
                    a + open.len()
                } else {
                    b + open.len() + close.len()
                };
                src.text = text;
                src.selection = (caret, caret);
                src.revision += 1;
                self.ui.source.set(src.clone());
                self.reparse(&src.text);
            }
            MarkupEdit::Insert(tag) => {
                let (a, b) = clamp_sel(&src.text, src.selection);
                let mut text = src.text.clone();
                text.replace_range(a..b, &tag);
                src.text = text;
                src.selection = (a + tag.len(), a + tag.len());
                src.revision += 1;
                self.ui.source.set(src.clone());
                self.reparse(&src.text);
            }
            MarkupEdit::ClearFormat => {
                let (a, b) = clamp_sel(&src.text, src.selection);
                let (a, b) = if a == b { (0, src.text.len()) } else { (a, b) };
                let inner = &src.text[a..b];
                let plain = strip_tags(inner);
                let mut text = src.text.clone();
                text.replace_range(a..b, &plain);
                src.text = text;
                src.selection = (a, a + plain.len());
                src.revision += 1;
                self.ui.source.set(src.clone());
                self.reparse(&src.text);
            }
        }
    }

    fn insert_asset(&mut self, asset: AssetRef) {
        self.tool(crate::msg::Tool::Insert(nanatype_model::Inline::Atom(
            nanatype_model::Atom {
                kind: nanatype_model::AtomKind::Image,
                asset,
                height_em: 2.0,
                alt: String::new(),
                effects: Default::default(),
            },
        )));
    }

    // ---- files ----------------------------------------------------------------

    fn pick(&mut self, target: FileTarget) {
        let id = self.next_dialog;
        self.next_dialog += 1;
        self.dialogs.insert(id, target);
        let t = |k: &str| self.ui.now_t(k);
        let images = FileFilter::new(
            t("tb-image").trim_end_matches('…').to_owned(),
            ["png", "jpg", "jpeg", "gif", "webp", "bmp"],
        );
        let skin_filter = FileFilter::new("NanaType skin", ["ntskin", "zip"]);
        let request = match target {
            FileTarget::Avatar => FileDialogRequest::new(id, FileDialogKind::OpenFile)
                .title(t("general-avatar-pick"))
                .filters([images]),
            FileTarget::InsertImage => FileDialogRequest::new(id, FileDialogKind::OpenFile)
                .title(t("tb-image"))
                .filters([images]),
            FileTarget::ImportSkin => FileDialogRequest::new(id, FileDialogKind::OpenFile)
                .title(t("skins-import"))
                .filters([skin_filter]),
            FileTarget::ExportSkin => FileDialogRequest::new(id, FileDialogKind::SaveFile)
                .title(t("skins-export"))
                .filters([skin_filter])
                .file_name(format!(
                    "{}.ntskin",
                    self.export_target.clone().unwrap_or_default()
                )),
            FileTarget::StickerFolder => {
                FileDialogRequest::new(id, FileDialogKind::PickFolder).title(t("assets-add-folder"))
            }
        };
        let window = [WindowKind::Settings, WindowKind::Input]
            .into_iter()
            .find(|k| self.windows.is_open(*k))
            .unwrap_or(WindowKind::Input);
        self.commands.push(WindowCommand::OpenFileDialog {
            id: window.id(),
            request,
        });
    }

    pub fn file_chosen(&mut self, request: u64, paths: Vec<PathBuf>) {
        let Some(target) = self.dialogs.remove(&request) else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        match target {
            FileTarget::Avatar => {
                let p = path.display().to_string();
                self.edit(|s| s.general.avatar = Some(p));
            }
            FileTarget::InsertImage => {
                self.insert_asset(AssetRef::File(path.display().to_string()))
            }
            FileTarget::ImportSkin => match self.registry.install(&path) {
                Ok(outcome) => {
                    self.refresh_skin_rows();
                    self.sync_tray_skins();
                    let (id, text) = match outcome {
                        nanatype_theme::InstallOutcome::Installed { id } => {
                            let t = self
                                .ui
                                .tr("skins-installed", &[("name", id.as_str().into())]);
                            (id, t)
                        }
                        nanatype_theme::InstallOutcome::Replaced { id, previous } => {
                            let t = self.ui.tr(
                                "skins-replaced",
                                &[("name", id.as_str().into()), ("previous", previous.into())],
                            );
                            (id, t)
                        }
                    };
                    self.notice(text, false);
                    if id == self.settings.skin.id {
                        self.reload_skin();
                    } else {
                        self.edit(|s| s.skin.id = id);
                    }
                }
                Err(errors) => {
                    let detail = errors.first().map(|d| d.to_string()).unwrap_or_default();
                    let text = format!("{}: {detail}", self.ui.now_t("skins-import-failed"));
                    self.notice(text, true);
                }
            },
            FileTarget::ExportSkin => {
                if let Some(id) = self.export_target.take()
                    && let Err(e) = self.registry.export(&id, &path)
                {
                    self.notice(e.to_string(), true);
                }
            }
            FileTarget::StickerFolder => {
                let p = path.display().to_string();
                self.edit(|s| {
                    if !s.assets.sticker_dirs.contains(&p) {
                        s.assets.sticker_dirs.push(p);
                    }
                });
            }
        }
    }

    // ---- update ---------------------------------------------------------------

    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Core(Cmd::Send) => {
                let fx = self.core.handle(Cmd::Send, clock::now_ms());
                // A send that took the draft: say so briefly.
                let sent = fx.replace_draft.as_ref().is_some_and(Document::is_blank);
                self.apply_effects(fx);
                if sent {
                    self.toasts.push(ToastRequest::new(
                        format!("{} ✓", self.ui.now_t("status-sent")),
                        nana_ui::ToastTone::Success,
                        feedback::SENT_MS,
                        false,
                    ));
                }
            }
            Msg::Core(cmd) => self.run(cmd),
            Msg::Edit(f) => self.edit(f),
            Msg::Markup(edit) => self.markup(edit),
            Msg::Tool(tool) => self.tool(tool),
            Msg::EditorCommand(command) => self.rich_commands.push(command),
            Msg::EditorChanged(rich) => self.editor_changed(rich),
            Msg::EditorPaste => {
                if let Ok(mut clipboard) = arboard::Clipboard::new()
                    && let Ok(image) = clipboard.get_image()
                {
                    let (w, h) = (image.width as u32, image.height as u32);
                    self.update(Msg::PasteImage {
                        width: w,
                        height: h,
                        rgba: image.bytes.into_owned(),
                    });
                }
            }
            Msg::ToggleSource => {
                let on = !self.ui.source_mode.get_untracked();
                self.ui.source_mode.set(on);
            }
            Msg::Hotkey(action) => self.hotkey(action),
            Msg::RecordHotkey(action) => {
                self.ui.recording.set(action);
                self.apply_services();
            }
            Msg::HotkeyCaptured(hotkey) => {
                if let Some(action) = self.ui.recording.get_untracked() {
                    self.ui.recording.set(None);
                    self.edit(|s| {
                        // A chord can belong to one action only.
                        let taken: Vec<GlobalAction> = GlobalAction::all_in(s)
                            .filter(|o| *o != action && o.get(s) == Some(&hotkey))
                            .collect();
                        for other in taken {
                            other.set(s, None);
                        }
                        action.set(s, Some(hotkey));
                    });
                    self.apply_services();
                }
            }
            Msg::HotkeyStatus(status) => self.ui.hotkeys.set(Arc::new(status)),
            Msg::ToggleWindow(kind) => match kind {
                WindowKind::Input
                    if self.settings.general.input_presence == InputPresence::Summon =>
                {
                    self.summoned = !self.summoned;
                }
                WindowKind::Settings => self.open_settings(),
                WindowKind::Chat if self.windows.is_open(WindowKind::Chat) => self.close_chat(),
                WindowKind::Chat => self.open_chat(),
                kind => self.edit(|s| {
                    if let Some(o) = kind.overlay_mut(s) {
                        o.visible = !o.visible;
                    }
                }),
            },
            Msg::ShowWindow(kind) => match kind {
                WindowKind::Input => {
                    self.summoned = true;
                    if !self.settings.overlays.input.visible {
                        self.edit(|s| s.overlays.input.visible = true);
                    }
                    self.commands
                        .push(WindowCommand::Focus(WindowKind::Input.id()));
                }
                WindowKind::Settings => self.open_settings(),
                WindowKind::Chat => self.open_chat(),
                kind => self.edit(|s| {
                    if let Some(o) = kind.overlay_mut(s) {
                        o.visible = true;
                    }
                }),
            },
            Msg::HideInput => self.summoned = false,
            Msg::ResetWindow(kind) => {
                self.windows.forget(kind);
                self.edit(|s| {
                    if let Some(o) = kind.overlay_mut(s) {
                        o.size = None;
                    }
                });
            }
            Msg::OpenSettings => self.open_settings(),
            Msg::SettingsPage(page) => {
                self.ui.page.set(page);
                self.open_settings();
            }
            Msg::PickFile(target) => self.pick(target),
            Msg::DeleteSkin(id) => {
                if let Err(e) = self.registry.remove(&id) {
                    self.notice(e.to_string(), true);
                }
                self.refresh_skin_rows();
                self.sync_tray_skins();
                if id == self.settings.skin.id {
                    self.edit(|s| s.skin.id = nanatype_theme::DEFAULT_SKIN.into());
                }
            }
            Msg::ExportSkin(id) => {
                self.export_target = Some(id);
                self.pick(FileTarget::ExportSkin);
            }
            Msg::DuplicateSkin(base) => self.duplicate_skin(&base),
            Msg::OpenFolder(path) => crate::services::reveal(&path),
            Msg::OpenUrl(url) => crate::services::open_url(url),
            Msg::CopyText(text) => {
                let ok = {
                    use nana_ui_platform::ClipboardHost;
                    nana_ui_platform::OsClipboard::new().write_text(&text)
                };
                if ok {
                    let t = self.ui.now_t("common-copied");
                    self.notice(t, false);
                }
            }
            Msg::PlaySample => {
                // Development aid: `NANATYPE_DEMO_MARKUP` plays that line instead.
                let line = std::env::var("NANATYPE_DEMO_MARKUP")
                    .ok()
                    .filter(|_| cfg!(debug_assertions))
                    .unwrap_or_else(|| self.ui.now_t("output-sample-line"));
                self.run(Cmd::SendMarkup(line));
            }
            Msg::VtsAuthorize => {
                if !self.settings.integrations.vts.enabled {
                    self.edit(|s| s.integrations.vts.enabled = true);
                }
                self.bridge.vts_request_token();
            }
            Msg::VtsRefresh => self.bridge.vts_refresh_hotkeys(),
            Msg::InsertPhrase(i) => {
                if let Some(p) = self.settings.assets.phrases.get(i).cloned() {
                    let speaker = self.settings.general.speaker_name.clone();
                    let expanded = nanatype_markup::expand_template(&p.markup, |k| match k {
                        "speaker" | "name" => Some(speaker.clone()),
                        "time" => Some(crate::ui::history::local_hm(nanatype_model::unix_millis())),
                        _ => None,
                    });
                    if p.send {
                        self.run(Cmd::SendMarkup(expanded));
                    } else if self.ui.source_mode.get_untracked() {
                        self.markup(MarkupEdit::Insert(expanded));
                    } else {
                        let source = self.ui.source.get_untracked().text;
                        self.set_source(format!("{source}{expanded}"), true);
                    }
                }
            }
            Msg::DismissNotice(id) => self.edit_status(|s| s.notices.retain(|n| n.id != id)),
            Msg::TrayErrorExpired(id) => {
                if self.tray_error.as_ref().is_some_and(|(e, _)| *e == id) {
                    self.tray_error = None;
                }
            }
            // The host removes toasts from its documents.
            Msg::ToastDismissed(..) => {}
            Msg::Wake { epoch, reason } => self.run(Cmd::Wake { epoch, reason }),
            Msg::Cue(kind) => self.run(Cmd::CueFired(kind)),
            Msg::Audio(status) => {
                let text = match &status {
                    AudioStatus::Running { device, rate } => format!("{device} · {rate} Hz"),
                    AudioStatus::Lost(e) | AudioStatus::Failed(e) => {
                        let t = self.ui.tr("err-audio", &[("message", e.as_str().into())]);
                        self.notice(t.clone(), true);
                        t
                    }
                };
                if matches!(status, AudioStatus::Running { .. }) {
                    // A fresh stream starts with an empty mixer.
                    sounds::install(&self.audio, &Vec::new(), &self.bank);
                    self.audio_levels();
                }
                let devices = nanatype_audio::list_devices();
                self.edit_status(|s| {
                    s.audio = text;
                    s.audio_devices = devices;
                });
            }
            Msg::Bridge(status) => self.bridge_status(status),
            Msg::Chat(message) => self.chat_message(message),
            Msg::ChatStatus(status) => self.chat_status(status),
            Msg::ChatMention { seq, reply } => self.mention(seq, reply),
            Msg::ClearReply => self.run(Cmd::SetReply(None)),
            Msg::ClearChat => {
                self.chat.clear();
                self.publish_chat();
            }
            Msg::AssetsChanged => self.refresh_assets(),
            Msg::SkinFilesChanged => {
                self.registry.rescan();
                self.reload_skin();
                self.refresh_skin_rows();
            }
            Msg::SecondInstance => {
                self.update(Msg::ShowWindow(WindowKind::Input));
                self.open_settings();
            }
            Msg::SoundsLoaded { skin, bank, errors } => {
                if skin == self.loaded.skin.meta.id {
                    sounds::install(&self.audio, &self.bank, &bank);
                    self.bank = bank;
                    if let Some(e) = errors.first() {
                        self.notice(e.clone(), true);
                    }
                }
            }
            Msg::Displays(monitors) => {
                self.windows.monitors = monitors;
            }
            Msg::Notice { text, error } => self.notice(text, error),
            Msg::InputHwnd(hwnd) => self.hotkeys.set_input_window(hwnd),
            Msg::PasteImage {
                width,
                height,
                rgba,
            } => match self.assets.store_rgba(width, height, &rgba) {
                Ok(asset) => self.insert_asset(asset),
                Err(e) => {
                    let text = self.ui.tr("err-image", &[("message", e.as_str().into())]);
                    self.notice(text, true);
                }
            },
            Msg::Quit => {
                self.quitting = true;
                self.save_geometry();
                self.persist.flush(Duration::from_secs(2));
            }
        }
    }

    fn duplicate_skin(&mut self, base: &str) {
        let mut n = 1;
        let id = loop {
            let candidate = format!(
                "{base}-copy{}",
                if n == 1 { String::new() } else { n.to_string() }
            );
            let candidate = candidate.replace("nanatype.", "my.");
            if self.registry.entry(&candidate).is_none() {
                break candidate;
            }
            n += 1;
        };
        let locale = self.ui.locale.get_untracked();
        let base_name = self
            .ui
            .skins
            .get_untracked()
            .iter()
            .find(|r| r.id == base)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| base.to_owned());
        let mut name = nanatype_theme::Localized::new();
        name.insert(locale.tag().to_owned(), format!("{base_name} *"));
        // Bake the current personalisation into the copy.
        let mut over = TomlTable::new();
        let o = &self.settings.skin.overrides;
        let mut dialogue = TomlTable::new();
        let mut text = TomlTable::new();
        if let Some(c) = o.text_color {
            text.insert("color".into(), c.to_hex().into());
        }
        if let Some(sz) = o.text_size {
            text.insert("size".into(), f64::from(sz).into());
        }
        if !text.is_empty() {
            dialogue.insert("text".into(), text.into());
        }
        if let Some(c) = o.box_color {
            let mut b = TomlTable::new();
            b.insert("color".into(), c.to_hex().into());
            dialogue.insert("box".into(), b.into());
        }
        if let Some(c) = o.name_color {
            let mut p = TomlTable::new();
            p.insert("color".into(), c.to_hex().into());
            dialogue.insert("nameplate".into(), p.into());
        }
        if !dialogue.is_empty() {
            over.insert("dialogue".into(), dialogue.into());
        }
        if let Some(c) = o.accent {
            let mut ch = TomlTable::new();
            ch.insert("accent".into(), c.to_hex().into());
            over.insert("chrome".into(), ch.into());
        }
        match self.registry.duplicate(base, &id, &name, &over) {
            Ok(dir) => {
                self.refresh_skin_rows();
                self.sync_tray_skins();
                self.edit(|s| {
                    s.skin.id = id;
                    s.skin.overrides = Default::default();
                });
                crate::services::reveal(&dir);
            }
            Err(errors) => {
                let detail = errors.first().map(|d| d.to_string()).unwrap_or_default();
                self.notice(detail, true);
            }
        }
    }

    fn bridge_status(&mut self, status: BridgeStatus) {
        let set = |app: &mut Self, sink: Sink, state: LinkState| {
            app.edit_status(|s| match sink {
                Sink::Vts => s.vts = state,
                Sink::Osc => s.osc = state,
                Sink::WebSocket => s.ws = state,
            });
        };
        match status {
            BridgeStatus::VtsToken(token) => self.edit(|s| s.integrations.vts.token = Some(token)),
            BridgeStatus::VtsTokenRejected => {
                self.edit(|s| s.integrations.vts.token = None);
                set(self, Sink::Vts, LinkState::NeedsAuth);
            }
            BridgeStatus::VtsHotkeys(keys) => {
                let list = keys.into_iter().map(|k| (k.id, k.name)).collect();
                self.edit_status(|s| s.vts_hotkeys = list);
            }
            BridgeStatus::VtsNeedsToken => set(self, Sink::Vts, LinkState::NeedsAuth),
            BridgeStatus::Clients(n) => {
                let v = self.ui.tr("int-ws-clients", &[("count", n.into())]);
                set(self, Sink::WebSocket, LinkState::Connected(v));
            }
            BridgeStatus::Off(sink) => set(self, sink, LinkState::Off),
            BridgeStatus::Connecting(sink) => set(self, sink, LinkState::Connecting),
            BridgeStatus::Connected(sink) => {
                let v = self.ui.now_t("int-status-connected");
                set(self, sink, LinkState::Connected(v));
            }
            BridgeStatus::Error(sink, message) => {
                let v = self
                    .ui
                    .tr("int-status-error", &[("message", message.into())]);
                set(self, sink, LinkState::Error(v));
            }
        }
    }

    // ---- live chat ----------------------------------------------------------

    fn chat_message(&mut self, message: Arc<ChatMessage>) {
        self.ensure_urls(message_assets(&message));
        let history = &self.settings.history.chat;
        if history.enabled && message.passes(&history.filter) {
            let locale = self.ui.locale.get_untracked();
            self.run(Cmd::External(message.history_item(locale)));
        }
        self.chat.push_back(message);
        while self.chat.len() > CHAT_CAP {
            self.chat.pop_front();
        }
        self.publish_chat();
    }

    fn publish_chat(&self) {
        self.ui
            .chat
            .set(Arc::new(self.chat.iter().cloned().collect()));
    }

    fn chat_status(&mut self, status: BliveStatus) {
        use crate::ui::danmaku::Link;
        let room = self.settings.integrations.blivechat.room.clone();
        let (text, link) = match status {
            BliveStatus::Off => (self.ui.now_t("int-status-off"), Link::Off),
            BliveStatus::Connecting => (self.ui.now_t("int-status-connecting"), Link::Connecting),
            BliveStatus::Connected => (
                self.ui
                    .tr("chat-status-joined", &[("room", room.as_str().into())]),
                Link::Connected,
            ),
            BliveStatus::Error(message) => (
                self.ui
                    .tr("chat-status-retrying", &[("message", message.into())]),
                Link::Error,
            ),
            BliveStatus::Fatal(_, message) => {
                let text = self.ui.tr("chat-fatal", &[("message", message.into())]);
                self.notice(text.clone(), true);
                (text, Link::Error)
            }
        };
        self.edit_status(|s| {
            s.blive = text;
            s.blive_link = link;
        });
    }

    /// Label the author of chat message `seq` at the editor's caret; a reply
    /// also quotes the message above the next line.
    fn mention(&mut self, seq: u64, reply: bool) {
        let Some(message) = self.chat.iter().rev().find(|m| m.seq == seq).cloned() else {
            return;
        };
        let mention = Inline::Mention(nanatype_model::Mention::new(message.user.clone()));
        self.tool(crate::msg::Tool::Insert(mention));
        // A space after the label, so typing goes on as text.
        if self.ui.source_mode.get_untracked() {
            self.markup(MarkupEdit::Insert(" ".into()));
        } else {
            self.rich_commands
                .push(nana_ui::runtime::RichEditCommand::InsertText(" ".into()));
        }
        if reply {
            let locale = self.ui.locale.get_untracked();
            self.run(Cmd::SetReply(Some(message.quote(locale))));
        }
        self.update(Msg::ShowWindow(WindowKind::Input));
    }

    pub fn open_chat(&mut self) {
        if !self.settings.integrations.blivechat.panel_open {
            self.edit(|s| s.integrations.blivechat.panel_open = true);
        }
        if self.windows.is_open(WindowKind::Chat) {
            if let Some(h) = self.windows.handle(WindowKind::Chat) {
                drop(h.set_visible(true));
                drop(h.focus());
            }
            return;
        }
        let title = self.ui.now_t(WindowKind::Chat.title_key());
        let desc = windows::descriptor(WindowKind::Chat, &title, &self.skin, &self.settings);
        self.commands.push(WindowCommand::Open {
            id: WindowKind::Chat.id(),
            settings: desc,
        });
    }

    /// The chat panel's width changed; rows re-wrap past a few pixels.
    pub fn chat_resized(&mut self, width: f32) {
        if (self.ui.chat_width.get_untracked() - width).abs() >= 4.0 {
            self.ui.chat_width.set(width);
        }
    }

    pub fn close_chat(&mut self) {
        if self.settings.integrations.blivechat.panel_open {
            self.edit(|s| s.integrations.blivechat.panel_open = false);
        }
        self.commands
            .push(WindowCommand::Close(WindowKind::Chat.id()));
    }

    fn hotkey(&mut self, action: GlobalAction) {
        match action {
            GlobalAction::Summon => {
                if self.settings.general.input_presence == InputPresence::Summon && self.summoned {
                    // The summon key toggles the input window in summon mode.
                    self.summoned = false;
                } else {
                    self.update(Msg::ShowWindow(WindowKind::Input));
                }
            }
            GlobalAction::Continue => self.run(Cmd::Continue),
            GlobalAction::Skip => self.run(Cmd::Skip),
            GlobalAction::Clear => self.run(Cmd::ClearDisplay),
            GlobalAction::ToggleMode => self.edit(|s| {
                s.general.input_mode = match s.general.input_mode {
                    nanatype_settings::InputMode::Compose => nanatype_settings::InputMode::Mirror,
                    nanatype_settings::InputMode::Mirror => nanatype_settings::InputMode::Compose,
                }
            }),
            GlobalAction::ToggleOverlays => self.edit(|s| {
                let show = !(s.overlays.display.visible || s.overlays.history.visible);
                s.overlays.display.visible = show;
                s.overlays.history.visible = show;
            }),
            GlobalAction::Phrase(i) => self.update(Msg::InsertPhrase(i)),
        }
    }

    pub fn open_settings(&mut self) {
        if self.windows.is_open(WindowKind::Settings) {
            self.commands
                .push(WindowCommand::Focus(WindowKind::Settings.id()));
            if let Some(h) = self.windows.handle(WindowKind::Settings) {
                drop(h.set_visible(true));
                drop(h.focus());
            }
            return;
        }
        let title = self.ui.now_t("window-settings");
        let desc = windows::descriptor(WindowKind::Settings, &title, &self.skin, &self.settings);
        self.commands.push(WindowCommand::Open {
            id: WindowKind::Settings.id(),
            settings: desc,
        });
    }

    /// Native window state for this frame.
    pub fn reconcile_windows(&mut self) -> Vec<WindowCommand> {
        let want: Vec<_> = WindowKind::ALL
            .into_iter()
            .map(|k| {
                (
                    k,
                    windows::desired(k, &self.skin, &self.settings, self.summoned),
                )
            })
            .collect();
        let mut out = std::mem::take(&mut self.commands);
        out.extend(self.windows.reconcile(&want));
        self.save_geometry();
        out
    }

    /// Motion is reduced when the setting or the system asks for it.
    pub fn sync_reduced_motion(&self) {
        let reduced =
            self.settings.playback.reduced_motion || self.ui.system_reduced_motion.get_untracked();
        if self.ui.reduced_motion.get_untracked() != reduced {
            self.ui.reduced_motion.set(reduced);
        }
    }

    pub fn update_result(&mut self) -> RuntimeProgramUpdate {
        // The input window rises in as it is summoned (see `ui::motion`).
        let shown = self.input_visible();
        if self.ui.input_shown.get_untracked() != shown {
            self.ui.input_shown.set(shown);
        }
        let window_commands = self.reconcile_windows();
        self.sync_tray();
        RuntimeProgramUpdate {
            window_commands,
            exit: self.quitting,
            ..RuntimeProgramUpdate::redraw_all()
        }
    }
}

/// Pictures `doc` shows: atoms and the avatars of mentioned viewers.
fn doc_assets(doc: &Document) -> Vec<AssetRef> {
    doc.paragraphs
        .iter()
        .flat_map(|p| &p.inlines)
        .filter_map(|i| match i {
            Inline::Atom(a) => Some(a.asset.clone()),
            Inline::Mention(m) => m.user.avatar.clone(),
            _ => None,
        })
        .collect()
}

fn display_assets(view: &nanatype_core::DisplayView) -> Vec<AssetRef> {
    let mut out = doc_assets(&view.doc);
    out.extend(view.reply.as_ref().and_then(|r| r.user.avatar.clone()));
    out
}

/// A history row's pictures: its text's, its avatar (chat rows name a
/// saved one) and the avatar of the message it answers.
fn row_assets(item: &nanatype_model::HistoryItem, doc: &Document) -> Vec<AssetRef> {
    let mut out = doc_assets(doc);
    out.extend(
        item.avatar
            .as_deref()
            .and_then(|a| a.parse::<AssetRef>().ok()),
    );
    out.extend(item.reply.as_ref().and_then(|r| r.user.avatar.clone()));
    out
}

fn message_assets(m: &ChatMessage) -> Vec<AssetRef> {
    let mut out = doc_assets(&m.doc);
    out.extend(m.user.avatar.clone());
    out
}

fn clamp_sel(text: &str, (a, b): (usize, usize)) -> (usize, usize) {
    let fix = |mut i: usize| {
        i = i.min(text.len());
        while i > 0 && !text.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let (a, b) = (fix(a), fix(b));
    (a.min(b), a.max(b))
}

/// Remove `[tag]` and `[/tag]` but keep `[[` escapes and the text.
fn strip_tags(text: &str) -> String {
    let parsed = parse(text);
    let mut doc = parsed.doc;
    for run in doc.runs_mut() {
        run.style = Default::default();
        run.effects = Default::default();
        run.pace = Default::default();
    }
    serialize(&doc.normalized())
}

fn quote(v: &str) -> String {
    if v.chars()
        .any(|c| c.is_whitespace() || matches!(c, ']' | '"' | '[' | '\\'))
    {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        v.to_owned()
    }
}

/// Debug demo: type a line through the editor's own command path (what the
/// toolbar and keyboard drive), then send it.
fn demo_editor(ctx: &RuntimeProgramContext<Msg>) {
    use crate::msg::Tool;
    use nana_ui::runtime::RichEditCommand as C;
    use nanatype_model::{ContinuousKind, Inline, Marker};
    let text = |t: &str| Msg::EditorCommand(C::InsertText(t.into()));
    let steps = [
        text("你好，这是"),
        Msg::Tool(Tool::Bold),
        text("粗体"),
        Msg::Tool(Tool::Bold),
        text("和"),
        Msg::Tool(Tool::Effect(ContinuousKind::Wave)),
        text("波浪"),
        Msg::Tool(Tool::Effect(ContinuousKind::Wave)),
        Msg::Tool(Tool::Insert(Inline::Marker(Marker::Wait { ms: 400 }))),
        Msg::Tool(Tool::Color("#e8505b".parse().expect("colour"))),
        text("红色！"),
    ];
    std::thread::sleep(Duration::from_millis(1500));
    for step in steps {
        ctx.dispatch(step);
        std::thread::sleep(Duration::from_millis(120));
    }
    std::thread::sleep(Duration::from_millis(2500));
    ctx.dispatch(Msg::Core(Cmd::Send));
}

/// The markup equivalent of a toolbar action (source mode).
fn markup_for(tool: &crate::msg::Tool) -> Option<MarkupEdit> {
    use crate::msg::Tool;
    let wrap = |open: String, close: &str| MarkupEdit::Wrap {
        open,
        close: close.to_owned(),
    };
    Some(match tool {
        Tool::Bold => wrap("[b]".into(), "[/b]"),
        Tool::Italic => wrap("[i]".into(), "[/i]"),
        Tool::Underline => wrap("[u]".into(), "[/u]"),
        Tool::Strike => wrap("[s]".into(), "[/s]"),
        Tool::Color(c) => wrap(format!("[color={c}]"), "[/color]"),
        Tool::Size(m) => wrap(format!("[size={m}]"), "[/size]"),
        Tool::Stroke(Some(s)) => wrap(format!("[stroke={},{}]", s.color, s.width_px), "[/stroke]"),
        Tool::Shadow(Some(s)) => wrap(
            format!("[shadow={},{},{},{}]", s.color, s.dx, s.dy, s.blur),
            "[/shadow]",
        ),
        Tool::Ruby(r) => wrap(format!("[ruby={}]", quote(r)), "[/ruby]"),
        Tool::Effect(k) => wrap(format!("[{}]", k.key()), &format!("[/{}]", k.key())),
        Tool::Entrance(k) => {
            let tag = match k {
                nanatype_model::EntranceKind::Fade => "fade",
                nanatype_model::EntranceKind::Pop => "pop",
                nanatype_model::EntranceKind::SlideUp => "slide",
                nanatype_model::EntranceKind::None => "noentrance",
            };
            wrap(format!("[{tag}]"), &format!("[/{tag}]"))
        }
        Tool::Pace(nanatype_model::Pace::Instant) => wrap("[instant]".into(), "[/instant]"),
        Tool::Pace(nanatype_model::Pace::Scale(s)) => wrap(format!("[speed={s}]"), "[/speed]"),
        Tool::Pace(nanatype_model::Pace::FixedMs(ms)) => {
            wrap(format!("[speed={ms}ms]"), "[/speed]")
        }
        Tool::Insert(inline) => {
            let doc = Document::from_inlines(vec![inline.clone()]);
            MarkupEdit::Insert(serialize(&doc))
        }
        Tool::ClearFormat => MarkupEdit::ClearFormat,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_clamp_to_char_boundaries() {
        assert_eq!(clamp_sel("你好", (1, 99)), (0, 6));
        assert_eq!(clamp_sel("abc", (2, 1)), (1, 2));
    }

    #[test]
    fn clearing_format_keeps_text_and_markers() {
        assert_eq!(
            strip_tags("[b]你[color=#ff0000]好[/color][/b][wait=300]"),
            "你好[wait=300]"
        );
    }

    #[test]
    fn quoting_only_when_needed() {
        assert_eq!(quote("cats/hi"), "cats/hi");
        assert_eq!(quote("file:C:/My Pics/a.png"), "\"file:C:/My Pics/a.png\"");
    }
}
