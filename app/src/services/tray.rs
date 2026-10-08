//! The notification-area icon and its menu. Labels follow the UI language;
//! check marks follow the settings. Menu clicks arrive on the UI thread's
//! message loop and are forwarded as [`Msg`]s.

use nanatype_i18n::{I18n, Locale};
use nanatype_settings::{InputMode, Lock, Settings};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::msg::Msg;
use crate::windows::WindowKind;

const ID_INPUT: &str = "nt.input";
const ID_DISPLAY: &str = "nt.display";
const ID_HISTORY: &str = "nt.history";
const ID_LOCK: &str = "nt.lock";
const ID_COMPOSE: &str = "nt.mode.compose";
const ID_MIRROR: &str = "nt.mode.mirror";
const ID_SETTINGS: &str = "nt.settings";
const ID_OBS: &str = "nt.obs";
const ID_QUIT: &str = "nt.quit";
const SKIN_PREFIX: &str = "nt.skin:";

pub struct Tray {
    _icon: TrayIcon,
    input: CheckMenuItem,
    display: CheckMenuItem,
    history: CheckMenuItem,
    lock: CheckMenuItem,
    mode: Submenu,
    compose: CheckMenuItem,
    mirror: CheckMenuItem,
    skins_menu: Submenu,
    skins: Vec<(String, CheckMenuItem)>,
    settings: MenuItem,
    obs: MenuItem,
    quit: MenuItem,
}

fn icon() -> Result<Icon, String> {
    let png = include_bytes!("../../assets/tray-32.png");
    let img = image::load_from_memory(png)
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).map_err(|e| e.to_string())
}

impl Tray {
    /// Build on the UI thread. `post` forwards menu actions.
    pub fn build(post: impl Fn(Msg) + Send + Sync + Clone + 'static) -> Result<Self, String> {
        let check = |id: &str| CheckMenuItem::with_id(id, "", true, false, None);
        let input = check(ID_INPUT);
        let display = check(ID_DISPLAY);
        let history = check(ID_HISTORY);
        let lock = check(ID_LOCK);
        let compose = check(ID_COMPOSE);
        let mirror = check(ID_MIRROR);
        let mode = Submenu::new("", true);
        mode.append_items(&[&compose, &mirror])
            .map_err(|e| e.to_string())?;
        let skins_menu = Submenu::new("", true);
        let settings = MenuItem::with_id(ID_SETTINGS, "", true, None);
        let obs = MenuItem::with_id(ID_OBS, "", true, None);
        let quit = MenuItem::with_id(ID_QUIT, "", true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &input,
            &display,
            &history,
            &lock,
            &PredefinedMenuItem::separator(),
            &mode,
            &skins_menu,
            &PredefinedMenuItem::separator(),
            &settings,
            &obs,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("NanaType")
            .with_icon(icon()?)
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| e.to_string())?;

        let menu_post = post.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let id = event.id.0.as_str();
            let msg = match id {
                ID_INPUT => Msg::ToggleWindow(WindowKind::Input),
                ID_DISPLAY => Msg::ToggleWindow(WindowKind::Display),
                ID_HISTORY => Msg::ToggleWindow(WindowKind::History),
                ID_LOCK => Msg::Edit(Box::new(|s: &mut Settings| {
                    let locked = s.overlays.display.lock == Lock::Locked;
                    let lock = if locked {
                        Lock::Interactive
                    } else {
                        Lock::Locked
                    };
                    s.overlays.display.lock = lock;
                    s.overlays.history.lock = lock;
                })),
                ID_COMPOSE => Msg::Edit(Box::new(|s: &mut Settings| {
                    s.general.input_mode = InputMode::Compose
                })),
                ID_MIRROR => Msg::Edit(Box::new(|s: &mut Settings| {
                    s.general.input_mode = InputMode::Mirror
                })),
                ID_SETTINGS => Msg::OpenSettings,
                ID_OBS => Msg::SettingsPage(crate::ui::settings::Page::Output),
                ID_QUIT => Msg::Quit,
                other => match other.strip_prefix(SKIN_PREFIX) {
                    Some(skin) => {
                        let skin = skin.to_owned();
                        Msg::Edit(Box::new(move |s: &mut Settings| s.skin.id = skin))
                    }
                    None => return,
                },
            };
            menu_post(msg);
        }));
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                post(Msg::ShowWindow(WindowKind::Input));
            }
        }));
        Ok(Self {
            _icon: icon,
            input,
            display,
            history,
            lock,
            mode,
            compose,
            mirror,
            skins_menu,
            skins: Vec::new(),
            settings,
            obs,
            quit,
        })
    }

    pub fn relabel(&self, i18n: &I18n, locale: Locale) {
        let t = |k: &str| i18n.t_in(locale, k);
        self.input.set_text(t("tray-show-input"));
        self.display.set_text(t("tray-show-display"));
        self.history.set_text(t("tray-show-history"));
        self.lock.set_text(t("tray-lock-overlays"));
        self.mode.set_text(t("tray-mode"));
        self.compose.set_text(t("mode-compose"));
        self.mirror.set_text(t("mode-mirror"));
        self.skins_menu.set_text(t("tray-skin"));
        self.settings.set_text(t("tray-settings"));
        self.obs.set_text(t("tray-obs-help"));
        self.quit.set_text(t("tray-quit"));
    }

    pub fn sync(&self, s: &Settings) {
        self.input.set_checked(s.overlays.input.visible);
        self.display.set_checked(s.overlays.display.visible);
        self.history.set_checked(s.overlays.history.visible);
        self.lock
            .set_checked(s.overlays.display.lock == Lock::Locked);
        self.compose
            .set_checked(s.general.input_mode == InputMode::Compose);
        self.mirror
            .set_checked(s.general.input_mode == InputMode::Mirror);
        for (id, item) in &self.skins {
            item.set_checked(*id == s.skin.id);
        }
    }

    /// Rebuild the skin submenu: `(id, display name)`.
    pub fn set_skins(&mut self, skins: &[(String, String)], active: &str) {
        for (_, item) in self.skins.drain(..) {
            let _ = self.skins_menu.remove(&item);
        }
        for (id, name) in skins {
            let item = CheckMenuItem::with_id(
                format!("{SKIN_PREFIX}{id}"),
                name,
                true,
                id == active,
                None,
            );
            let _ = self.skins_menu.append(&item);
            self.skins.push((id.clone(), item));
        }
    }
}
