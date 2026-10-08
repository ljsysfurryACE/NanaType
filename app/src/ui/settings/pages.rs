use std::sync::Arc;

use nana_ui::icons_tabler as ti;
use nana_ui::runtime::view::{AnyView, IntoView, dynamic, widget};
use nana_ui::runtime::{Activate, Button, IconButton, Stack, StatusBadge, Text};
use nana_ui::{ButtonKind, ControlSize, StatusTone};
use nana_ui_core::{AlignSpec, LengthSpec, SemanticColorRole};
use nanatype_i18n::Locale;
use nanatype_model::{AfterLine, EntranceKind};
use nanatype_playback::QueuePolicy;
use nanatype_settings::{
    CaptureBackground, ChromeMode, HistorySync, InputMode, InputPresence, Lock, QuickPhrase,
    SendKey, Settings,
};

use super::Page;
use super::controls::*;
use crate::msg::{FileTarget, GlobalAction, Msg};
use crate::services::hotkeys::HotkeyState;
use crate::ui::{I18N, Ui};
use crate::windows::WindowKind;

pub fn page(ui: Ui, page: Page) -> AnyView {
    let body = match page {
        Page::General => general(ui),
        Page::Playback => playback(ui),
        Page::Skins => skins(ui),
        Page::Windows => windows(ui),
        Page::History => history(ui),
        Page::Hotkeys => hotkeys(ui),
        Page::Output => output(ui),
        Page::Integrations => integrations(ui),
        Page::Assets => assets(ui),
        Page::About => about(ui),
    };
    widget(Stack::column(4.0))
        .children((heading(ui, page.label_key()), body))
        .into_any()
}

fn button(ui: Ui, key: &str, msg: impl Fn() -> Msg + Send + 'static) -> AnyView {
    widget(Button::new(ui.now_t(key)).size(ControlSize::Small))
        .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(msg()))
        .into_any()
}

fn primary(ui: Ui, key: &str, msg: impl Fn() -> Msg + Send + 'static) -> AnyView {
    widget(
        Button::new(ui.now_t(key))
            .kind(ButtonKind::Primary)
            .size(ControlSize::Small),
    )
    .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(msg()))
    .into_any()
}

fn general(ui: Ui) -> AnyView {
    let mut langs = vec![("system".to_owned(), ui.now_t("general-language-system"))];
    langs.extend(
        Locale::ALL
            .iter()
            .map(|l| (l.tag().to_owned(), l.native_name().to_owned())),
    );
    let avatar_row = {
        let has = ui.settings.get_untracked().general.avatar.is_some();
        widget(Stack::row(8.0))
            .children((
                button(ui, "general-avatar-pick", || {
                    Msg::PickFile(FileTarget::Avatar)
                }),
                has.then(|| {
                    button(ui, "general-avatar-clear", || {
                        edit(|s| s.general.avatar = None)
                    })
                }),
            ))
            .into_any()
    };
    let welcome = (!ui.settings.get_untracked().general.onboarding_done).then(|| {
        widget(
            Stack::column(8.0)
                .padding_xy(18.0, 16.0)
                .surface(SemanticColorRole::Selected)
                .radius(nana_ui_core::RadiusTier::Lg)
                .with_layout(|l| l.margin_top = Some(LengthSpec::Px(12.0))),
        )
        .children((
            widget(Text::new(ui.now_t("onboard-welcome")).style(text_style(18.0, 700))),
            hint(ui.now_t("onboard-welcome-body")),
            hint({
                let s = ui.settings.get_untracked();
                let label = |h: Option<&nanatype_settings::Hotkey>| {
                    h.map(|h| h.label("Win")).unwrap_or_else(|| "—".into())
                };
                I18N.tr_in(
                    ui.locale.get_untracked(),
                    "onboard-hotkeys-body",
                    &[
                        ("summon", label(s.hotkeys.summon.as_ref()).into()),
                        ("continue", label(s.hotkeys.continue_.as_ref()).into()),
                    ],
                )
            }),
            widget(Stack::row(8.0)).children((
                primary(ui, "onboard-done", || {
                    edit(|s| s.general.onboarding_done = true)
                }),
                button(ui, "onboard-obs", || Msg::SettingsPage(Page::Output)),
            )),
        ))
    });
    widget(Stack::column(0.0))
        .children((
            welcome,
            section(
                ui,
                "settings-general",
                vec![
                    row(
                        ui,
                        "general-language",
                        None,
                        select(
                            ui,
                            langs,
                            |s| s.general.locale.clone().unwrap_or_else(|| "system".into()),
                            |s, v| s.general.locale = (v != "system").then(|| v.to_owned()),
                        ),
                    ),
                    row(
                        ui,
                        "general-speaker",
                        Some("general-speaker-hint"),
                        text(
                            ui,
                            String::new(),
                            220.0,
                            |s| s.general.speaker_name.clone(),
                            |s, v| s.general.speaker_name = v,
                        ),
                    ),
                    row(ui, "general-avatar", None, avatar_row),
                ],
            ),
            section(
                ui,
                "general-presence",
                vec![
                    row(
                        ui,
                        "general-presence",
                        Some("general-presence-hint"),
                        select(
                            ui,
                            select_keys(
                                ui,
                                &[
                                    ("resident", "general-presence-resident"),
                                    ("summon", "general-presence-summon"),
                                ],
                            ),
                            |s| match s.general.input_presence {
                                InputPresence::Resident => "resident".into(),
                                InputPresence::Summon => "summon".into(),
                            },
                            |s, v| {
                                s.general.input_presence = if v == "summon" {
                                    InputPresence::Summon
                                } else {
                                    InputPresence::Resident
                                }
                            },
                        ),
                    ),
                    row(
                        ui,
                        "general-mode",
                        None,
                        select(
                            ui,
                            select_keys(
                                ui,
                                &[("compose", "mode-compose"), ("mirror", "mode-mirror")],
                            ),
                            |s| match s.general.input_mode {
                                InputMode::Compose => "compose".into(),
                                InputMode::Mirror => "mirror".into(),
                            },
                            |s, v| {
                                s.general.input_mode = if v == "mirror" {
                                    InputMode::Mirror
                                } else {
                                    InputMode::Compose
                                }
                            },
                        ),
                    ),
                    row(
                        ui,
                        "general-send-key",
                        None,
                        select(
                            ui,
                            select_keys(
                                ui,
                                &[
                                    ("enter", "general-send-enter"),
                                    ("ctrl", "general-send-ctrl-enter"),
                                ],
                            ),
                            |s| match s.general.send_key {
                                SendKey::Enter => "enter".into(),
                                SendKey::CtrlEnter => "ctrl".into(),
                            },
                            |s, v| {
                                s.general.send_key = if v == "ctrl" {
                                    SendKey::CtrlEnter
                                } else {
                                    SendKey::Enter
                                }
                            },
                        ),
                    ),
                    row(
                        ui,
                        "general-autostart",
                        None,
                        switch(
                            ui,
                            |s| s.general.launch_at_login,
                            |s, v| s.general.launch_at_login = v,
                        ),
                    ),
                ],
            ),
        ))
        .into_any()
}

fn after_key(a: &AfterLine) -> String {
    match a {
        AfterLine::Hold => "hold".into(),
        AfterLine::Fade { .. } => "fade".into(),
        AfterLine::Clear { .. } => "clear".into(),
    }
}

fn after_delay(a: &AfterLine) -> f64 {
    match a {
        AfterLine::Fade { delay_ms, .. } | AfterLine::Clear { delay_ms } => {
            f64::from(*delay_ms) / 1000.0
        }
        AfterLine::Hold => 3.0,
    }
}

fn playback(ui: Ui) -> AnyView {
    let entrance_opts = select_keys(
        ui,
        &[
            ("skin", "tb-entrance-skin"),
            ("none", "tb-entrance-none"),
            ("fade", "tb-entrance-fade"),
            ("pop", "tb-entrance-pop"),
            ("slide", "tb-entrance-slide"),
        ],
    );
    let mut devices = vec![("default".to_owned(), ui.now_t("playback-device-default"))];
    devices.extend(
        ui.status
            .get_untracked()
            .audio_devices
            .iter()
            .map(|d| (d.clone(), d.clone())),
    );
    widget(Stack::column(0.0))
        .children((
            section(
                ui,
                "settings-playback",
                vec![
                    row(
                        ui,
                        "playback-typing",
                        Some("playback-typing-hint"),
                        switch(ui, |s| s.playback.typing, |s, v| s.playback.typing = v),
                    ),
                    row(
                        ui,
                        "playback-speed",
                        None,
                        slider(
                            ui,
                            (0.25, 3.0, 0.05),
                            "×",
                            |s| f64::from(1.0 / s.playback.speed_scale.max(0.01)),
                            |s, v| s.playback.speed_scale = (1.0 / v.max(0.01)) as f32,
                        ),
                    ),
                    row(
                        ui,
                        "playback-punct",
                        Some("playback-punct-hint"),
                        switch(
                            ui,
                            |s| s.playback.punctuation_pauses,
                            |s, v| s.playback.punctuation_pauses = v,
                        ),
                    ),
                    row(
                        ui,
                        "playback-entrance",
                        None,
                        select(
                            ui,
                            entrance_opts,
                            |s| {
                                s.playback
                                    .entrance
                                    .map(|e| e.key().to_owned())
                                    .unwrap_or_else(|| "skin".into())
                            },
                            |s, v| s.playback.entrance = EntranceKind::from_key(v),
                        ),
                    ),
                ],
            ),
            section(
                ui,
                "playback-after",
                vec![
                    row(
                        ui,
                        "playback-after",
                        None,
                        select(
                            ui,
                            select_keys(
                                ui,
                                &[
                                    ("hold", "playback-after-hold"),
                                    ("fade", "playback-after-fade"),
                                    ("clear", "playback-after-clear"),
                                ],
                            ),
                            |s| after_key(&s.playback.after),
                            |s, v| {
                                let delay = (after_delay(&s.playback.after) * 1000.0) as u32;
                                s.playback.after = match v {
                                    "fade" => AfterLine::Fade {
                                        delay_ms: delay,
                                        per_unit_ms: 40,
                                    },
                                    "clear" => AfterLine::Clear { delay_ms: delay },
                                    _ => AfterLine::Hold,
                                }
                            },
                        ),
                    ),
                    row(
                        ui,
                        "playback-after-delay",
                        None,
                        slider(
                            ui,
                            (0.5, 30.0, 0.5),
                            "s",
                            |s| after_delay(&s.playback.after),
                            |s, v| {
                                let ms = (v * 1000.0) as u32;
                                match &mut s.playback.after {
                                    AfterLine::Fade { delay_ms, .. }
                                    | AfterLine::Clear { delay_ms } => *delay_ms = ms,
                                    AfterLine::Hold => {}
                                }
                            },
                        ),
                    ),
                    row(
                        ui,
                        "playback-auto",
                        None,
                        slider(
                            ui,
                            (0.0, 10.0, 0.5),
                            "s",
                            |s| {
                                s.playback
                                    .auto_advance_ms
                                    .map(|v| f64::from(v) / 1000.0)
                                    .unwrap_or(0.0)
                            },
                            |s, v| {
                                s.playback.auto_advance_ms =
                                    (v > 0.0).then_some((v * 1000.0) as u32)
                            },
                        ),
                    ),
                    row(
                        ui,
                        "playback-queue",
                        None,
                        select(
                            ui,
                            select_keys(
                                ui,
                                &[
                                    ("replace", "playback-queue-replace"),
                                    ("finish", "playback-queue-finish"),
                                    ("queue", "playback-queue-queue"),
                                ],
                            ),
                            |s| match s.playback.queue {
                                QueuePolicy::Replace => "replace".into(),
                                QueuePolicy::FinishThenNext => "finish".into(),
                                QueuePolicy::Queue => "queue".into(),
                            },
                            |s, v| {
                                s.playback.queue = match v {
                                    "finish" => QueuePolicy::FinishThenNext,
                                    "queue" => QueuePolicy::Queue,
                                    _ => QueuePolicy::Replace,
                                }
                            },
                        ),
                    ),
                    row(
                        ui,
                        "playback-reduced",
                        Some("playback-reduced-hint"),
                        switch(
                            ui,
                            |s| s.playback.reduced_motion,
                            |s, v| s.playback.reduced_motion = v,
                        ),
                    ),
                    row(
                        ui,
                        "playback-stagger",
                        None,
                        slider(
                            ui,
                            (0.0, 120.0, 5.0),
                            "ms",
                            |s| f64::from(s.playback.mirror_stagger_ms),
                            |s, v| s.playback.mirror_stagger_ms = v as u32,
                        ),
                    ),
                ],
            ),
            section(
                ui,
                "playback-sound",
                vec![
                    row(
                        ui,
                        "playback-sound",
                        None,
                        switch(ui, |s| s.sound.typing, |s, v| s.sound.typing = v),
                    ),
                    row(
                        ui,
                        "playback-volume",
                        None,
                        slider(
                            ui,
                            (0.0, 1.0, 0.01),
                            "",
                            |s| f64::from(s.sound.master),
                            |s, v| s.sound.master = v as f32,
                        ),
                    ),
                    row(
                        ui,
                        "playback-volume-typing",
                        None,
                        slider(
                            ui,
                            (0.0, 1.0, 0.01),
                            "",
                            |s| f64::from(s.sound.typing_volume),
                            |s, v| s.sound.typing_volume = v as f32,
                        ),
                    ),
                    row(
                        ui,
                        "playback-volume-effects",
                        None,
                        slider(
                            ui,
                            (0.0, 1.0, 0.01),
                            "",
                            |s| f64::from(s.sound.effects_volume),
                            |s, v| s.sound.effects_volume = v as f32,
                        ),
                    ),
                    row(
                        ui,
                        "playback-device",
                        None,
                        select(
                            ui,
                            devices,
                            |s| s.sound.device.clone().unwrap_or_else(|| "default".into()),
                            |s, v| s.sound.device = (v != "default").then(|| v.to_owned()),
                        ),
                    ),
                ],
            ),
        ))
        .into_any()
}

fn skins(ui: Ui) -> AnyView {
    let list = dynamic(
        move || {
            (
                ui.skins.with(|s| Arc::as_ptr(s) as usize),
                ui.settings.with(|s| s.skin.id.clone()),
            )
        },
        move |(_, active)| {
            let rows: Vec<AnyView> = ui
                .skins
                .get_untracked()
                .iter()
                .map(|skin| {
                    let id = skin.id.clone();
                    let selected = skin.id == *active;
                    let swatch = |color: [f32; 4]| {
                        widget(
                            Stack::row(0.0)
                                .width(LengthSpec::Px(14.0))
                                .height(LengthSpec::Px(14.0))
                                .with_layout(move |l| {
                                    l.background = Some(color);
                                    l.border_radius = Some(7.0);
                                    l.border_width = Some(1.0);
                                    l.border_color = Some([0.5, 0.5, 0.5, 0.5]);
                                    l.border_style = Some(nana_ui_core::BorderStyle::Solid);
                                }),
                        )
                        .into_any()
                    };
                    let detail = match &skin.error {
                        Some(e) => format!("{} · {e}", ui.now_t("skins-broken")),
                        None if skin.builtin => ui.now_t("skins-builtin"),
                        None => format!(
                            "{} · {}",
                            I18N.tr_in(
                                ui.locale.get_untracked(),
                                "skins-by",
                                &[("author", skin.author.as_str().into())]
                            ),
                            I18N.tr_in(
                                ui.locale.get_untracked(),
                                "skins-version",
                                &[("version", skin.version.as_str().into())]
                            )
                        ),
                    };
                    let usable = skin.error.is_none();
                    let (eid, did, rid) = (id.clone(), id.clone(), id.clone());
                    let folder = skin.folder.clone();
                    let builtin = skin.builtin;
                    let actions = widget(Stack::row(4.0)).children((
                        widget(
                            IconButton::new(ti::COPY, ui.now_t("skins-duplicate"))
                                .size(ControlSize::Small)
                                .with_tooltip(ui.now_t("skins-duplicate")),
                        )
                        .on_cx(move |_, _: &Activate, cx| {
                            cx.dispatch_program_all(Msg::DuplicateSkin(did.clone()))
                        }),
                        widget(
                            IconButton::new(ti::FILE_EXPORT, ui.now_t("skins-export"))
                                .size(ControlSize::Small)
                                .with_tooltip(ui.now_t("skins-export")),
                        )
                        .on_cx(move |_, _: &Activate, cx| {
                            cx.dispatch_program_all(Msg::ExportSkin(eid.clone()))
                        }),
                        folder.map(|f| {
                            widget(
                                IconButton::new(ti::FOLDER_OPEN, ui.now_t("skins-open-folder"))
                                    .size(ControlSize::Small)
                                    .with_tooltip(ui.now_t("skins-open-folder")),
                            )
                            .on_cx(move |_, _: &Activate, cx| {
                                cx.dispatch_program_all(Msg::OpenFolder(f.clone()))
                            })
                        }),
                        (!builtin).then(|| {
                            widget(
                                IconButton::new(ti::TRASH, ui.now_t("skins-delete"))
                                    .size(ControlSize::Small)
                                    .with_tooltip(ui.now_t("skins-delete")),
                            )
                            .on_cx(move |_, _: &Activate, cx| {
                                cx.dispatch_program_all(Msg::DeleteSkin(rid.clone()))
                            })
                        }),
                    ));
                    let use_id = id.clone();
                    widget(
                        Stack::bar(12.0)
                            .padding_xy(16.0, 10.0)
                            .align(AlignSpec::Center)
                            .with_layout(move |l| {
                                if selected {
                                    l.background = Some([0.5, 0.5, 0.5, 0.08]);
                                }
                            }),
                    )
                    .children((
                        widget(Stack::row(4.0)).children((
                            swatch(skin.box_color),
                            swatch(skin.text_color),
                            swatch(skin.accent),
                        )),
                        widget(Stack::column(2.0).width(LengthSpec::Fill))
                            .children((label(skin.name.clone()), hint(detail))),
                        actions,
                        if selected {
                            widget(
                                StatusBadge::new(ui.now_t("skins-active"), StatusTone::Success)
                                    .compact(true),
                            )
                            .into_any()
                        } else {
                            widget(
                                Button::new(ui.now_t("common-apply"))
                                    .size(ControlSize::Small)
                                    .disabled(!usable),
                            )
                            .on_cx(move |_, _: &Activate, cx| {
                                let id = use_id.clone();
                                cx.dispatch_program_all(edit(move |s| s.skin.id = id));
                            })
                            .into_any()
                        },
                    ))
                    .into_any()
                })
                .collect();
            widget(Stack::column(0.0)).children(rows).into_any()
        },
    );
    let fallback = ui.skin.get_untracked();
    widget(Stack::column(0.0))
        .children((
            widget(Stack::row(8.0).with_layout(|l| l.margin_top = Some(LengthSpec::Px(12.0))))
                .children(primary(ui, "skins-import", || {
                    Msg::PickFile(FileTarget::ImportSkin)
                })),
            section(ui, "skins-active", vec![list.into_any()]),
            section(
                ui,
                "skins-overrides",
                vec![
                    row(
                        ui,
                        "skins-override-text",
                        None,
                        color(
                            ui,
                            fallback.dialogue.text.color,
                            |s| s.skin.overrides.text_color,
                            |s, v| s.skin.overrides.text_color = v,
                        ),
                    ),
                    row(
                        ui,
                        "skins-override-name",
                        None,
                        color(
                            ui,
                            fallback.dialogue.nameplate.color,
                            |s| s.skin.overrides.name_color,
                            |s, v| s.skin.overrides.name_color = v,
                        ),
                    ),
                    row(
                        ui,
                        "skins-override-box",
                        None,
                        color(
                            ui,
                            fallback.dialogue.box_.color,
                            |s| s.skin.overrides.box_color,
                            |s, v| s.skin.overrides.box_color = v,
                        ),
                    ),
                    row(
                        ui,
                        "skins-override-accent",
                        None,
                        color(
                            ui,
                            fallback.chrome.accent,
                            |s| s.skin.overrides.accent,
                            |s, v| s.skin.overrides.accent = v,
                        ),
                    ),
                    row(
                        ui,
                        "skins-override-size",
                        None,
                        slider(
                            ui,
                            (12.0, 72.0, 1.0),
                            "px",
                            |s| f64::from(s.skin.overrides.text_size.unwrap_or(0.0)),
                            |s, v| s.skin.overrides.text_size = (v >= 12.0).then_some(v as f32),
                        ),
                    ),
                    row(
                        ui,
                        "skins-override-reset",
                        None,
                        button(ui, "common-reset", || {
                            edit(|s| s.skin.overrides = Default::default())
                        }),
                    ),
                ],
            ),
            section(
                ui,
                "skins-chrome",
                vec![
                    row(
                        ui,
                        "skins-chrome",
                        None,
                        select(
                            ui,
                            select_keys(
                                ui,
                                &[
                                    ("skin", "skins-chrome-skin"),
                                    ("system", "skins-chrome-system"),
                                    ("light", "skins-chrome-light"),
                                    ("dark", "skins-chrome-dark"),
                                ],
                            ),
                            |s| match s.skin.chrome {
                                ChromeMode::Skin => "skin".into(),
                                ChromeMode::System => "system".into(),
                                ChromeMode::Light => "light".into(),
                                ChromeMode::Dark => "dark".into(),
                            },
                            |s, v| {
                                s.skin.chrome = match v {
                                    "system" => ChromeMode::System,
                                    "light" => ChromeMode::Light,
                                    "dark" => ChromeMode::Dark,
                                    _ => ChromeMode::Skin,
                                }
                            },
                        ),
                    ),
                    row(
                        ui,
                        "skins-hot-reload",
                        Some("skins-hot-reload-hint"),
                        switch(ui, |s| s.skin.hot_reload, |s, v| s.skin.hot_reload = v),
                    ),
                ],
            ),
        ))
        .into_any()
}

fn window_section(ui: Ui, kind: WindowKind, title: &'static str) -> AnyView {
    let get_overlay = move |s: &Settings| kind.overlay(s).cloned().unwrap_or_default();
    let lock_opts = select_keys(
        ui,
        &[
            ("locked", "windows-lock-locked"),
            ("interactive", "windows-lock-interactive"),
            ("edit", "windows-lock-edit"),
        ],
    );
    let bg_opts = select_keys(
        ui,
        &[
            ("transparent", "windows-background-transparent"),
            ("chroma", "windows-background-chroma"),
        ],
    );
    let mut rows = vec![row(
        ui,
        "windows-visible",
        None,
        match kind {
            WindowKind::Input => switch(
                ui,
                |s| s.overlays.input.visible,
                |s, v| s.overlays.input.visible = v,
            ),
            WindowKind::Display => switch(
                ui,
                |s| s.overlays.display.visible,
                |s, v| s.overlays.display.visible = v,
            ),
            _ => switch(
                ui,
                |s| s.overlays.history.visible,
                |s, v| s.overlays.history.visible = v,
            ),
        },
    )];
    if kind != WindowKind::Input {
        rows.push(row(
            ui,
            "windows-lock",
            None,
            select(
                ui,
                lock_opts,
                move |s| match get_overlay(s).lock {
                    Lock::Locked => "locked".into(),
                    Lock::Interactive => "interactive".into(),
                    Lock::EditLayout => "edit".into(),
                },
                move |s, v| {
                    if let Some(o) = kind.overlay_mut(s) {
                        o.lock = match v {
                            "interactive" => Lock::Interactive,
                            "edit" => Lock::EditLayout,
                            _ => Lock::Locked,
                        }
                    }
                },
            ),
        ));
        rows.push(row(
            ui,
            "windows-background",
            None,
            select(
                ui,
                bg_opts,
                move |s| match get_overlay(s).background {
                    CaptureBackground::Transparent => "transparent".into(),
                    CaptureBackground::Chroma { .. } => "chroma".into(),
                },
                move |s, v| {
                    if let Some(o) = kind.overlay_mut(s) {
                        o.background = if v == "chroma" {
                            CaptureBackground::Chroma {
                                color: nanatype_model::Rgba8::rgb(0, 255, 0),
                            }
                        } else {
                            CaptureBackground::Transparent
                        }
                    }
                },
            ),
        ));
    }
    rows.push(row(
        ui,
        "windows-on-top",
        None,
        match kind {
            WindowKind::Input => switch(
                ui,
                |s| s.overlays.input.always_on_top,
                |s, v| s.overlays.input.always_on_top = v,
            ),
            WindowKind::Display => switch(
                ui,
                |s| s.overlays.display.always_on_top,
                |s, v| s.overlays.display.always_on_top = v,
            ),
            _ => switch(
                ui,
                |s| s.overlays.history.always_on_top,
                |s, v| s.overlays.history.always_on_top = v,
            ),
        },
    ));
    rows.push(row(
        ui,
        "windows-reset",
        None,
        button(ui, "common-reset", move || Msg::ResetWindow(kind)),
    ));
    section(ui, title, rows)
}

fn windows(ui: Ui) -> AnyView {
    widget(Stack::column(0.0))
        .children((
            window_section(ui, WindowKind::Display, "windows-display"),
            window_section(ui, WindowKind::History, "windows-history"),
            window_section(ui, WindowKind::Input, "windows-input"),
        ))
        .into_any()
}

fn history(ui: Ui) -> AnyView {
    widget(Stack::column(0.0))
        .children((section(
            ui,
            "settings-history",
            vec![
                row(
                    ui,
                    "history-sync",
                    None,
                    select(
                        ui,
                        select_keys(
                            ui,
                            &[
                                ("submit", "history-sync-submit"),
                                ("reveal", "history-sync-reveal"),
                                ("complete", "history-sync-complete"),
                            ],
                        ),
                        |s| match s.history.sync {
                            HistorySync::OnSubmit => "submit".into(),
                            HistorySync::Reveal => "reveal".into(),
                            HistorySync::OnComplete => "complete".into(),
                        },
                        |s, v| {
                            s.history.sync = match v {
                                "submit" => HistorySync::OnSubmit,
                                "complete" => HistorySync::OnComplete,
                                _ => HistorySync::Reveal,
                            }
                        },
                    ),
                ),
                row(
                    ui,
                    "history-visible",
                    None,
                    number(
                        ui,
                        (1.0, 200.0),
                        |s| f64::from(s.history.visible_items),
                        |s, v| s.history.visible_items = v as u32,
                    ),
                ),
                row(
                    ui,
                    "history-max",
                    None,
                    number(
                        ui,
                        (1.0, 5000.0),
                        |s| f64::from(s.history.max_items),
                        |s, v| s.history.max_items = v as u32,
                    ),
                ),
                row(
                    ui,
                    "history-fade",
                    None,
                    slider(
                        ui,
                        (0.0, 300.0, 5.0),
                        "s",
                        |s| f64::from(s.history.fade_after_s.unwrap_or(0)),
                        |s, v| s.history.fade_after_s = (v > 0.0).then_some(v as u32),
                    ),
                ),
                row(
                    ui,
                    "history-avatar",
                    None,
                    switch(
                        ui,
                        |s| s.history.show_avatar,
                        |s, v| s.history.show_avatar = v,
                    ),
                ),
                row(
                    ui,
                    "history-name",
                    None,
                    switch(ui, |s| s.history.show_name, |s, v| s.history.show_name = v),
                ),
                row(
                    ui,
                    "history-time",
                    None,
                    switch(ui, |s| s.history.show_time, |s, v| s.history.show_time = v),
                ),
                row(
                    ui,
                    "history-clear",
                    None,
                    button(ui, "history-clear", || {
                        Msg::Core(nanatype_core::Cmd::ClearHistory)
                    }),
                ),
            ],
        ),))
        .into_any()
}

fn hotkeys(ui: Ui) -> AnyView {
    let rows: Vec<AnyView> = GlobalAction::ALL
        .into_iter()
        .map(|action| {
            let label = move || {
                let locale = ui.locale.get();
                if ui.recording.get() == Some(action) {
                    return I18N.t_in(locale, "hotkeys-recording");
                }
                ui.settings
                    .with(|s| action.get(s).cloned())
                    .map(|h| {
                        h.label(if cfg!(target_os = "macos") {
                            "⌘"
                        } else {
                            "Win"
                        })
                    })
                    .unwrap_or_else(|| I18N.t_in(locale, "hotkeys-none"))
            };
            let badge = move || {
                let locale = ui.locale.get();
                let bound = ui.settings.with(|s| action.get(s).is_some());
                match (bound, ui.hotkeys.with(|h| h.get(&action).cloned())) {
                    (true, Some(HotkeyState::Registered)) => {
                        (I18N.t_in(locale, "hotkeys-ok"), StatusTone::Success)
                    }
                    (true, Some(HotkeyState::Conflict)) => {
                        (I18N.t_in(locale, "hotkeys-conflict-os"), StatusTone::Danger)
                    }
                    (true, Some(HotkeyState::Invalid)) => {
                        (I18N.t_in(locale, "hotkeys-unsafe"), StatusTone::Warning)
                    }
                    _ => (String::new(), StatusTone::Neutral),
                }
            };
            let control = widget(Stack::row(8.0).align(AlignSpec::Center)).children((
                widget(StatusBadge::new("", StatusTone::Neutral).compact(true))
                    .visible(move || !badge().0.is_empty())
                    .bind(move |b: &mut StatusBadge| {
                        let (text, tone) = badge();
                        let text: std::sync::Arc<str> = text.into();
                        if b.label != text {
                            b.label = text;
                        }
                        b.tone = tone;
                    }),
                widget(Button::new("").size(ControlSize::Small))
                    .bind(move |b: &mut Button| {
                        let text = label();
                        if b.label != text {
                            b.label = text;
                        }
                        b.kind = if ui.recording.get() == Some(action) {
                            ButtonKind::Primary
                        } else {
                            ButtonKind::Subtle
                        };
                    })
                    .on_cx(move |_, _: &Activate, cx| {
                        let recording = ui.recording.get_untracked() == Some(action);
                        cx.dispatch_program_all(Msg::RecordHotkey(if recording {
                            None
                        } else {
                            Some(action)
                        }))
                    }),
                widget(
                    IconButton::new(ti::X, ui.now_t("hotkeys-clear-binding"))
                        .size(ControlSize::Small),
                )
                .on_cx(move |_, _: &Activate, cx| {
                    cx.dispatch_program_all(edit(move |s| action.set(s, None)))
                }),
            ));
            row(ui, action.label_key(), None, control.into_any())
        })
        .collect();
    widget(Stack::column(0.0))
        .children((
            section(ui, "settings-hotkeys", rows),
            section(
                ui,
                "hotkeys-local",
                vec![
                    widget(Stack::column(0.0).padding_xy(16.0, 12.0))
                        .children(hint(ui.now_t("hotkeys-local-hint")))
                        .into_any(),
                ],
            ),
        ))
        .into_any()
}

fn output(ui: Ui) -> AnyView {
    let spout = |kind: WindowKind, title: &'static str| {
        let status = move || {
            ui.status.with(|s| match kind {
                WindowKind::Display => s.spout_display.clone(),
                _ => s.spout_history.clone(),
            })
        };
        let name = move |s: &Settings| match kind {
            WindowKind::Display => s.output.display.name.clone(),
            _ => s.output.history.name.clone(),
        };
        let copy_name = name(&ui.settings.get_untracked());
        section(
            ui,
            title,
            vec![
                row(
                    ui,
                    "output-spout",
                    None,
                    match kind {
                        WindowKind::Display => switch(
                            ui,
                            |s| s.output.display.enabled,
                            |s, v| s.output.display.enabled = v,
                        ),
                        _ => switch(
                            ui,
                            |s| s.output.history.enabled,
                            |s, v| s.output.history.enabled = v,
                        ),
                    },
                ),
                row(
                    ui,
                    "output-spout-name",
                    None,
                    widget(Stack::row(6.0))
                        .children((
                            match kind {
                                WindowKind::Display => text(
                                    ui,
                                    String::new(),
                                    220.0,
                                    |s| s.output.display.name.clone(),
                                    |s, v| s.output.display.name = v,
                                ),
                                _ => text(
                                    ui,
                                    String::new(),
                                    220.0,
                                    |s| s.output.history.name.clone(),
                                    |s, v| s.output.history.name = v,
                                ),
                            },
                            widget(
                                IconButton::new(ti::COPY, ui.now_t("output-spout-copy"))
                                    .size(ControlSize::Small)
                                    .with_tooltip(ui.now_t("output-spout-copy")),
                            )
                            .on_cx(move |_, _: &Activate, cx| {
                                cx.dispatch_program_all(Msg::CopyText(copy_name.clone()))
                            }),
                        ))
                        .into_any(),
                ),
                row(
                    ui,
                    "output-spout-size",
                    None,
                    select(
                        ui,
                        vec![
                            ("window".into(), ui.now_t("output-spout-size-window")),
                            ("1280x720".into(), "1280 × 720".into()),
                            ("1920x1080".into(), "1920 × 1080".into()),
                            ("2560x1440".into(), "2560 × 1440".into()),
                            ("3840x2160".into(), "3840 × 2160".into()),
                        ],
                        move |s| {
                            let o = match kind {
                                WindowKind::Display => &s.output.display,
                                _ => &s.output.history,
                            };
                            o.size
                                .map(|[w, h]| format!("{w}x{h}"))
                                .unwrap_or_else(|| "window".into())
                        },
                        move |s, v| {
                            let size = v
                                .split_once('x')
                                .and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?]));
                            match kind {
                                WindowKind::Display => s.output.display.size = size,
                                _ => s.output.history.size = size,
                            }
                        },
                    ),
                ),
                row(
                    ui,
                    "output-spout-straight",
                    Some("output-spout-straight-hint"),
                    match kind {
                        WindowKind::Display => switch(
                            ui,
                            |s| s.output.display.straight_alpha,
                            |s, v| s.output.display.straight_alpha = v,
                        ),
                        _ => switch(
                            ui,
                            |s| s.output.history.straight_alpha,
                            |s, v| s.output.history.straight_alpha = v,
                        ),
                    },
                ),
                row_text(
                    String::new(),
                    None,
                    widget(Text::new("").color(SemanticColorRole::Muted))
                        .bind(move |t: &mut Text| {
                            let v = status();
                            if t.value != v {
                                t.value = v;
                            }
                        })
                        .into_any(),
                ),
            ],
        )
    };
    let obs = vec![
        widget(Stack::column(8.0).padding_xy(16.0, 12.0))
            .children((
                hint(ui.now_t(if cfg!(windows) {
                    "output-obs-spout"
                } else {
                    "output-spout-unsupported"
                })),
                hint(ui.now_t(if cfg!(target_os = "macos") {
                    "output-obs-mac"
                } else {
                    "output-obs-window"
                })),
                widget(Stack::row(0.0)).children(primary(ui, "output-sample", || Msg::PlaySample)),
            ))
            .into_any(),
    ];
    widget(Stack::column(0.0))
        .children((
            section(ui, "output-obs-title", obs),
            spout(WindowKind::Display, "output-display"),
            spout(WindowKind::History, "output-history"),
        ))
        .into_any()
}

fn status_text(ui: Ui, get: fn(&crate::ui::Status) -> String) -> AnyView {
    widget(Text::new("").color(SemanticColorRole::Muted))
        .bind(move |t: &mut Text| {
            let v = ui.status.with(|s| get(s));
            if t.value != v {
                t.value = v;
            }
        })
        .into_any()
}

fn integrations(ui: Ui) -> AnyView {
    let mapping = dynamic(
        move || ui.status.with(|s| s.vts_hotkeys.clone()),
        move |hotkeys| {
            if hotkeys.is_empty() {
                return widget(Stack::column(0.0).padding_xy(16.0, 10.0))
                    .children(hint(ui.now_t("int-vts-map-auto")))
                    .into_any();
            }
            let rows: Vec<AnyView> = hotkeys
                .iter()
                .map(|(id, name)| {
                    let id = id.clone();
                    let name = name.clone();
                    let expr = name.to_lowercase().replace(' ', "_");
                    let tag = format!("[expr={expr}]");
                    row_text(
                        name.clone(),
                        Some(tag.clone()),
                        widget(Stack::row(6.0))
                            .children((
                                widget(
                                    IconButton::new(ti::COPY, ui.now_t("output-spout-copy"))
                                        .size(ControlSize::Small),
                                )
                                .on_cx(
                                    move |_, _: &Activate, cx| {
                                        cx.dispatch_program_all(Msg::CopyText(tag.clone()))
                                    },
                                ),
                                widget(
                                    Button::new(ui.now_t("common-apply")).size(ControlSize::Small),
                                )
                                .on_cx(
                                    move |_, _: &Activate, cx| {
                                        let (e, i) = (expr.clone(), id.clone());
                                        cx.dispatch_program_all(edit(move |s| {
                                            s.integrations.vts.expressions.insert(e, i);
                                        }))
                                    },
                                ),
                            ))
                            .into_any(),
                    )
                })
                .collect();
            widget(Stack::column(0.0)).children(rows).into_any()
        },
    );
    widget(Stack::column(0.0))
        .children((
            section(
                ui,
                "int-vts",
                vec![
                    row(
                        ui,
                        "int-vts",
                        Some("int-vts-hint"),
                        switch(
                            ui,
                            |s| s.integrations.vts.enabled,
                            |s, v| s.integrations.vts.enabled = v,
                        ),
                    ),
                    row(
                        ui,
                        "int-vts-port",
                        None,
                        number(
                            ui,
                            (1.0, 65535.0),
                            |s| f64::from(s.integrations.vts.port),
                            |s, v| s.integrations.vts.port = v as u16,
                        ),
                    ),
                    row(
                        ui,
                        "int-vts-authorize",
                        Some("int-vts-authorize-hint"),
                        widget(Stack::row(8.0))
                            .children((
                                status_text(ui, |s| s.vts.clone()),
                                button(ui, "int-vts-authorize", || Msg::VtsAuthorize),
                                button(ui, "int-vts-refresh", || Msg::VtsRefresh),
                            ))
                            .into_any(),
                    ),
                    mapping.into_any(),
                ],
            ),
            section(
                ui,
                "int-osc",
                vec![
                    row(
                        ui,
                        "int-osc",
                        Some("int-osc-hint"),
                        switch(
                            ui,
                            |s| s.integrations.osc.enabled,
                            |s, v| s.integrations.osc.enabled = v,
                        ),
                    ),
                    row(
                        ui,
                        "int-osc-host",
                        None,
                        text(
                            ui,
                            "127.0.0.1".into(),
                            180.0,
                            |s| s.integrations.osc.host.clone(),
                            |s, v| s.integrations.osc.host = v,
                        ),
                    ),
                    row(
                        ui,
                        "int-osc-port",
                        None,
                        number(
                            ui,
                            (1.0, 65535.0),
                            |s| f64::from(s.integrations.osc.port),
                            |s, v| s.integrations.osc.port = v as u16,
                        ),
                    ),
                    row(
                        ui,
                        "int-osc-prefix",
                        None,
                        text(
                            ui,
                            "/nanatype".into(),
                            180.0,
                            |s| s.integrations.osc.prefix.clone(),
                            |s, v| s.integrations.osc.prefix = v,
                        ),
                    ),
                    row_text(String::new(), None, status_text(ui, |s| s.osc.clone())),
                ],
            ),
            section(
                ui,
                "int-ws",
                vec![
                    row(
                        ui,
                        "int-ws",
                        Some("int-ws-hint"),
                        switch(
                            ui,
                            |s| s.integrations.websocket.enabled,
                            |s, v| s.integrations.websocket.enabled = v,
                        ),
                    ),
                    row(
                        ui,
                        "int-ws-port",
                        None,
                        number(
                            ui,
                            (1024.0, 65535.0),
                            |s| f64::from(s.integrations.websocket.port),
                            |s, v| s.integrations.websocket.port = v as u16,
                        ),
                    ),
                    row_text(String::new(), None, status_text(ui, |s| s.ws.clone())),
                ],
            ),
        ))
        .into_any()
}

fn assets(ui: Ui) -> AnyView {
    let dirs = dynamic(
        move || ui.settings.with(|s| s.assets.sticker_dirs.clone()),
        move |dirs| {
            let rows: Vec<AnyView> = dirs
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    row_text(
                        d.clone(),
                        None,
                        widget(
                            IconButton::new(ti::TRASH, ui.now_t("assets-remove"))
                                .size(ControlSize::Small),
                        )
                        .on_cx(move |_, _: &Activate, cx| {
                            cx.dispatch_program_all(edit(move |s| {
                                if i < s.assets.sticker_dirs.len() {
                                    s.assets.sticker_dirs.remove(i);
                                }
                            }))
                        })
                        .into_any(),
                    )
                })
                .collect();
            widget(Stack::column(0.0)).children(rows).into_any()
        },
    );
    let phrases = dynamic(
        move || ui.settings.with(|s| s.assets.phrases.len()),
        move |n| {
            let rows: Vec<AnyView> = (0..*n)
                .map(|i| {
                    let p = ui.settings.with_untracked(|s| s.assets.phrases[i].clone());
                    widget(
                        Stack::bar(8.0)
                            .padding_xy(16.0, 8.0)
                            .align(AlignSpec::Center),
                    )
                    .children((
                        widget(Stack::row(0.0).width(LengthSpec::Px(140.0))).children(
                            widget(
                                nana_ui::runtime::TextInput::new(p.label.clone())
                                    .size(ControlSize::Small)
                                    .placeholder(ui.now_t("assets-phrase-label")),
                            )
                            .on_cx(
                                move |_, e: &nana_ui::runtime::TextChanged, cx| {
                                    let v = e.value.to_string();
                                    cx.dispatch_program_all(edit(move |s| {
                                        if let Some(p) = s.assets.phrases.get_mut(i) {
                                            p.label = v;
                                        }
                                    }))
                                },
                            ),
                        ),
                        widget(Stack::row(0.0).width(LengthSpec::Fill)).children(
                            widget(
                                nana_ui::runtime::TextInput::new(p.markup.clone())
                                    .size(ControlSize::Small)
                                    .placeholder(ui.now_t("assets-phrase-markup")),
                            )
                            .on_cx(
                                move |_, e: &nana_ui::runtime::TextChanged, cx| {
                                    let v = e.value.to_string();
                                    cx.dispatch_program_all(edit(move |s| {
                                        if let Some(p) = s.assets.phrases.get_mut(i) {
                                            p.markup = v;
                                        }
                                    }))
                                },
                            ),
                        ),
                        widget(nana_ui::runtime::Switch::new(
                            ui.now_t("assets-phrase-send"),
                            p.send,
                        ))
                        .on_cx(
                            move |_, e: &nana_ui::runtime::ToggleChanged, cx| {
                                let v = e.checked;
                                cx.dispatch_program_all(edit(move |s| {
                                    if let Some(p) = s.assets.phrases.get_mut(i) {
                                        p.send = v;
                                    }
                                }))
                            },
                        ),
                        widget(
                            IconButton::new(ti::TRASH, ui.now_t("assets-remove"))
                                .size(ControlSize::Small),
                        )
                        .on_cx(move |_, _: &Activate, cx| {
                            cx.dispatch_program_all(edit(move |s| {
                                if i < s.assets.phrases.len() {
                                    s.assets.phrases.remove(i);
                                }
                            }))
                        }),
                    ))
                    .into_any()
                })
                .collect();
            widget(Stack::column(0.0)).children(rows).into_any()
        },
    );
    widget(Stack::column(0.0))
        .children((
            section(
                ui,
                "assets-stickers",
                vec![
                    widget(Stack::column(0.0).padding_xy(16.0, 10.0))
                        .children(hint(ui.now_t("assets-stickers-hint")))
                        .into_any(),
                    dirs.into_any(),
                    widget(Stack::row(0.0).padding_xy(16.0, 10.0))
                        .children(button(ui, "assets-add-folder", || {
                            Msg::PickFile(FileTarget::StickerFolder)
                        }))
                        .into_any(),
                ],
            ),
            section(
                ui,
                "assets-phrases",
                vec![
                    phrases.into_any(),
                    widget(Stack::row(0.0).padding_xy(16.0, 10.0))
                        .children(button(ui, "assets-phrase-add", || {
                            edit(|s| {
                                s.assets.phrases.push(QuickPhrase {
                                    label: String::new(),
                                    markup: String::new(),
                                    send: false,
                                    hotkey: None,
                                })
                            })
                        }))
                        .into_any(),
                ],
            ),
        ))
        .into_any()
}

fn about(ui: Ui) -> AnyView {
    let locale = ui.locale.get_untracked();
    let version = I18N.tr_in(
        locale,
        "about-version",
        &[("version", env!("CARGO_PKG_VERSION").into())],
    );
    let build = I18N.tr_in(
        locale,
        "about-build",
        &[("build", env!("NANATYPE_BUILD_ID").into())],
    );
    let paths = crate::paths::Paths::resolve();
    let (logs, data) = (paths.logs.clone(), paths.data.clone());
    widget(Stack::column(0.0))
        .children(section(
            ui,
            "settings-about",
            vec![
                widget(Stack::column(4.0).padding_xy(16.0, 14.0))
                    .children((
                        widget(Text::new("NanaType").style(text_style(20.0, 700))),
                        hint(ui.now_t("app-tagline")),
                        hint(version),
                        hint(build),
                    ))
                    .into_any(),
                row(
                    ui,
                    "about-logs",
                    None,
                    button(ui, "about-logs", move || Msg::OpenFolder(logs.clone())),
                ),
                row(
                    ui,
                    "about-data",
                    None,
                    button(ui, "about-data", move || Msg::OpenFolder(data.clone())),
                ),
            ],
        ))
        .into_any()
}
