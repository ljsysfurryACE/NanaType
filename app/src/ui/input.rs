//! The input window: a toolbar over the dialogue box, and the streamer types
//! straight into that box. The editor is NanaUI's `RichTextEditor` styled
//! with the skin's dialogue text, so it lays out exactly as the typewriter
//! will show it. Toolbar buttons send semantic [`Tool`] actions; a source
//! toggle swaps the editor for the markup text of the same draft.

use std::sync::Arc;

use nana_ui::icons_tabler as ti;
use nana_ui::runtime::view::{
    AnyView, EntityRef, IntoView, Signal, WithRefs, dynamic, entity_ref, segmented,
    segmented_option, signal, widget, with_refs,
};
use nana_ui::runtime::{
    ActionMenu, ActionMenuItem, Activate, Button, ColorChanged, ColorField, Divider, IconButton,
    NodeStyle, Popover, PopoverClosed, PopoverToggled, RangeField, RangeInput, RichAttrMask,
    RichSelectionAttrs, RichTextEditor, RichTextEditorEvent, Stack, Text, TextArea, TextChanged,
    TextInput, TextSubmitted,
};
use nana_ui::{ButtonKind, ControlSize, PopoverPlacement};
use nana_ui_core::{AlignSpec, LengthSpec, PointerEventsSpec, PositionSpec, SemanticColorRole};
use nanatype_core::Cmd;
use nanatype_i18n::{Arg, Locale};
use nanatype_model::{
    AssetRef, Atom, AtomKind, ContinuousKind, EntranceKind, Inline, Marker, Pace, Rgba8, Shadow,
    Stroke, VtsAction,
};
use nanatype_settings::{InputMode, SendKey, Settings};

use super::dialogue::{Base, Source, dialogue_box};
use super::{I18N, Ui, c, restyle};
use crate::msg::{FileTarget, MarkupEdit, Msg, Tool};
use crate::windows::{STATUS_H, TOOLBAR_H, WindowKind};

pub fn marker_label(m: &Marker, locale: Locale) -> String {
    let t = |k: &str| I18N.t_in(locale, k);
    let tr = |k: &str, name: &str, v: Arg<'_>| I18N.tr_in(locale, k, &[(name, v)]);
    match m {
        Marker::Wait { ms } => tr("marker-wait", "ms", (*ms).into()),
        Marker::Pause { auto_ms: None } => t("marker-pause"),
        Marker::Pause { auto_ms: Some(ms) } => tr("marker-pause-auto", "ms", (*ms).into()),
        Marker::Page => t("marker-page"),
        Marker::Sound { sound, .. } => tr("marker-sound", "name", sound.as_str().into()),
        Marker::Expression { name, .. } => tr("marker-expression", "name", name.as_str().into()),
        Marker::Portrait { variant } => tr("marker-portrait", "name", variant.as_str().into()),
        Marker::ScreenShake { .. } => t("marker-shake"),
        Marker::Vts(action) => {
            let name = match action {
                VtsAction::Hotkey { hotkey } => hotkey,
                VtsAction::Expression { file, .. } => file,
                VtsAction::Param { name, .. } => name,
            };
            tr("marker-vts", "name", name.as_str().into())
        }
    }
}

fn act(tool: Tool) -> Msg {
    Msg::Tool(tool)
}

fn marker(m: Marker) -> Msg {
    Msg::Tool(Tool::Insert(Inline::Marker(m)))
}

/// An icon button with a tooltip that sends `msg()`.
fn tool(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    label: &'static str,
    msg: impl Fn() -> Msg + Send + 'static,
) -> AnyView {
    widget(
        IconButton::new(icon, ui.now_t(label))
            .size(ControlSize::Small)
            .with_tooltip(ui.now_t(label)),
    )
    .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(msg()))
    .into_any()
}

/// A toolbar toggle (bold, italic…) shown pressed while the whole selection
/// has the attribute.
fn toggle(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    label: &'static str,
    tool_of: fn() -> Tool,
    pressed: fn(&RichSelectionAttrs) -> bool,
) -> AnyView {
    widget(
        IconButton::new(icon, ui.now_t(label))
            .size(ControlSize::Small)
            .with_tooltip(ui.now_t(label)),
    )
    .bind(move |b: &mut IconButton| {
        let on = !ui.source_mode.get() && ui.sel_attrs.with(pressed);
        if b.selected != on {
            b.selected = on;
        }
    })
    .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(act(tool_of())))
    .into_any()
}

fn is_bold(a: &RichSelectionAttrs) -> bool {
    !a.mixed.contains(RichAttrMask::WEIGHT) && a.style.shape.weight.is_some_and(|w| w >= 600)
}

fn is_italic(a: &RichSelectionAttrs) -> bool {
    !a.mixed.contains(RichAttrMask::ITALIC) && a.style.shape.italic == Some(true)
}

fn is_underline(a: &RichSelectionAttrs) -> bool {
    !a.mixed.contains(RichAttrMask::DECORATION)
        && a.style.paint.decoration.is_some_and(|d| d.underline)
}

fn is_strike(a: &RichSelectionAttrs) -> bool {
    !a.mixed.contains(RichAttrMask::DECORATION)
        && a.style.paint.decoration.is_some_and(|d| d.line_through)
}

/// Swaps the editor for the draft's markup source.
fn source_toggle(ui: Ui) -> AnyView {
    widget(
        IconButton::new(ti::CODE, ui.now_t("tb-source"))
            .size(ControlSize::Small)
            .with_tooltip(ui.now_t("tb-source")),
    )
    .bind(move |b: &mut IconButton| {
        let on = ui.source_mode.get();
        if b.selected != on {
            b.selected = on;
        }
    })
    .on_cx(|_, _: &Activate, cx| cx.dispatch_program_all(Msg::ToggleSource))
    .into_any()
}

/// A popover whose open state the app keeps, so items can close it.
fn popover(
    icon: nana_ui::runtime::Icon,
    label: String,
    width: f32,
    open: Signal<bool>,
    body: impl IntoView,
) -> AnyView {
    widget(
        Popover::new()
            .trigger_icon(icon, label)
            .placement(PopoverPlacement::Bottom)
            .width(width)
            .bare_trigger(true),
    )
    .bind(move |p: &mut Popover| p.open = open.get())
    .on(move |e: &PopoverToggled| open.set(e.open))
    .on(move |_: &PopoverClosed| open.set(false))
    .children(body)
    .into_any()
}

/// Menu item that sends `msg()` and closes its menu.
fn item(label: String, open: Signal<bool>, msg: impl Fn() -> Msg + Send + 'static) -> AnyView {
    widget(ActionMenuItem::new(label))
        .on_cx(move |_, _: &Activate, cx| {
            open.set(false);
            cx.dispatch_program_all(msg());
        })
        .into_any()
}

fn menu(
    icon: nana_ui::runtime::Icon,
    label: String,
    open: Signal<bool>,
    items: Vec<AnyView>,
) -> AnyView {
    widget(
        ActionMenu::new()
            .trigger_icon(icon, label)
            .bare_trigger(true),
    )
    .bind(move |m: &mut ActionMenu| m.popover.open = open.get())
    .on(move |e: &PopoverToggled| open.set(e.open))
    .on(move |_: &PopoverClosed| open.set(false))
    .children(items)
    .into_any()
}

/// A small button painted with a colour.
fn swatch_button(label: String, color: [f32; 4]) -> Button {
    let mut st = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.width = Some(LengthSpec::Px(26.0));
        l.height = Some(LengthSpec::Px(26.0));
        l.min_width = Some(LengthSpec::Px(26.0));
        l.background = Some(color);
        l.border_radius = Some(6.0);
        l.border_width = Some(1.0);
        l.border_color = Some([0.5, 0.5, 0.5, 0.45]);
        l.border_style = Some(nana_ui_core::BorderStyle::Solid);
        l.padding_left = Some(LengthSpec::Px(0.0));
        l.padding_right = Some(LengthSpec::Px(0.0));
    }
    Button::new("")
        .accessible_name(label)
        .kind(ButtonKind::Text)
        .style(st)
}

/// A button showing an image (sticker picker).
fn image_button(label: String, url: String) -> Button {
    let mut st = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.width = Some(LengthSpec::Px(52.0));
        l.height = Some(LengthSpec::Px(52.0));
        l.border_radius = Some(6.0);
        l.paint.background_image = Some(nana_ui_core::BackgroundImage::url_with_fit(
            url,
            nana_ui_core::BackgroundImageFit::Contain,
        ));
    }
    Button::new("")
        .accessible_name(label)
        .kind(ButtonKind::Text)
        .style(st)
}

const SWATCHES: [&str; 12] = [
    "#e8505b", "#f08a3c", "#f7c948", "#7bc96f", "#2fb5a6", "#3d9be9", "#5b6be8", "#a066e0",
    "#e86a92", "#ffffff", "#8a8f98", "#1b1f27",
];

fn color_panel(ui: Ui, open: Signal<bool>) -> AnyView {
    let rows: Vec<AnyView> = SWATCHES
        .chunks(6)
        .map(|chunk| {
            let buttons: Vec<AnyView> = chunk
                .iter()
                .map(|hex| {
                    let rgba: Rgba8 = hex.parse().expect("swatch");
                    let color = super::c(rgba);
                    widget(swatch_button(hex.to_string(), color))
                        .on_cx(move |_, _: &Activate, cx| {
                            open.set(false);
                            cx.dispatch_program_all(act(Tool::Color(rgba)));
                        })
                        .into_any()
                })
                .collect();
            widget(Stack::row(6.0)).children(buttons).into_any()
        })
        .collect();
    let picked = signal([0.9_f32, 0.3, 0.4, 1.0]);
    widget(Stack::column(8.0).padding(4.0))
        .children((
            rows,
            widget(ColorField::new([0.9, 0.3, 0.4, 1.0]).label(ui.now_t("tb-color")))
                .on(move |e: &ColorChanged| picked.set(e.value)),
            widget(Stack::row(6.0)).children((widget(
                Button::new(ui.now_t("common-apply"))
                    .kind(ButtonKind::Primary)
                    .size(ControlSize::Small),
            )
            .on_cx(move |_, _: &Activate, cx| {
                let [r, g, b, a] = picked.get_untracked();
                let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                let rgba = Rgba8::rgba(to(r), to(g), to(b), to(a));
                open.set(false);
                cx.dispatch_program_all(act(Tool::Color(rgba)));
            }),)),
        ))
        .into_any()
}

fn size_panel(ui: Ui, open: Signal<bool>) -> AnyView {
    let value = signal(1.5_f64);
    let presets: Vec<AnyView> = [0.75_f32, 1.25, 1.5, 2.0]
        .into_iter()
        .map(|v| {
            widget(Button::new(format!("{v}×")).size(ControlSize::Small))
                .on_cx(move |_, _: &Activate, cx| {
                    open.set(false);
                    cx.dispatch_program_all(act(Tool::Size(v)));
                })
                .into_any()
        })
        .collect();
    widget(Stack::column(8.0).padding(4.0))
        .children((
            widget(Stack::row(6.0)).children(presets),
            widget(
                RangeField::new(1.5, 0.5, 3.0, 0.05)
                    .label(ui.now_t("tb-size"))
                    .unit("×"),
            )
            .on(move |e: &RangeInput| value.set(e.value)),
            widget(
                Button::new(ui.now_t("common-apply"))
                    .kind(ButtonKind::Primary)
                    .size(ControlSize::Small),
            )
            .on_cx(move |_, _: &Activate, cx| {
                let v = (value.get_untracked() * 100.0).round() / 100.0;
                open.set(false);
                cx.dispatch_program_all(act(Tool::Size(v as f32)));
            }),
        ))
        .into_any()
}

fn ruby_panel(ui: Ui, open: Signal<bool>) -> AnyView {
    let reading = signal(String::new());
    widget(Stack::column(8.0).padding(4.0))
        .children((
            widget(TextInput::new("").placeholder(ui.now_t("tb-ruby-placeholder")))
                .on(move |e: &TextChanged| reading.set(e.value.to_string()))
                .on_cx(move |_, _: &TextSubmitted, cx| {
                    let r = reading.get_untracked();
                    if !r.trim().is_empty() {
                        open.set(false);
                        cx.dispatch_program_all(act(Tool::Ruby(r)));
                    }
                }),
            widget(Stack::row(6.0)).children((
                widget(
                    Button::new(ui.now_t("tb-ruby-apply"))
                        .kind(ButtonKind::Primary)
                        .size(ControlSize::Small),
                )
                .on_cx(move |_, _: &Activate, cx| {
                    let r = reading.get_untracked();
                    if !r.trim().is_empty() {
                        open.set(false);
                        cx.dispatch_program_all(act(Tool::Ruby(r)));
                    }
                }),
                widget(Button::new(ui.now_t("tb-ruby-remove")).size(ControlSize::Small)).on_cx(
                    move |_, _: &Activate, cx| {
                        open.set(false);
                        cx.dispatch_program_all(act(Tool::Ruby(String::new())));
                    },
                ),
            )),
        ))
        .into_any()
}

fn expression_panel(ui: Ui, open: Signal<bool>) -> AnyView {
    let name = signal(String::new());
    let skin = ui.skin.get_untracked();
    let variants: Vec<AnyView> = skin
        .dialogue
        .portrait
        .variants
        .keys()
        .map(|v| {
            let v = v.clone();
            let label = v.clone();
            widget(Button::new(label).size(ControlSize::Small))
                .on_cx(move |_, _: &Activate, cx| {
                    open.set(false);
                    cx.dispatch_program_all(marker(Marker::Expression {
                        name: v.clone(),
                        intensity: None,
                    }));
                })
                .into_any()
        })
        .collect();
    widget(Stack::column(8.0).padding(4.0))
        .children((
            widget(Stack::row(6.0).wrap(true)).children(variants),
            widget(TextInput::new("").placeholder(ui.now_t("tb-expression-placeholder")))
                .on(move |e: &TextChanged| name.set(e.value.to_string()))
                .on_cx(move |_, _: &TextSubmitted, cx| {
                    let n = name.get_untracked();
                    if !n.trim().is_empty() {
                        open.set(false);
                        cx.dispatch_program_all(marker(Marker::Expression {
                            name: n.trim().to_owned(),
                            intensity: None,
                        }));
                    }
                }),
        ))
        .into_any()
}

fn sticker_panel(ui: Ui, open: Signal<bool>) -> AnyView {
    let query = signal(String::new());
    widget(Stack::column(8.0).padding(4.0))
        .children((
            widget(TextInput::new("").placeholder(ui.now_t("tb-sticker-search")))
                .on(move |e: &TextChanged| query.set(e.value.to_string())),
            dynamic(
                move || (query.get(), ui.stickers.with(|s| Arc::as_ptr(s) as usize)),
                move |(q, _)| {
                    let stickers = ui.stickers.get_untracked();
                    if stickers.is_empty() {
                        return super::feedback::empty_state(
                            ui.now_t("empty-stickers"),
                            Some((
                                ui.now_t("empty-open-assets"),
                                Box::new(move || {
                                    open.set(false);
                                    Msg::SettingsPage(super::settings::Page::Assets)
                                }),
                            )),
                        );
                    }
                    let q = q.to_lowercase();
                    let cells: Vec<AnyView> = stickers
                        .iter()
                        .filter(|s| {
                            q.is_empty()
                                || s.name.to_lowercase().contains(&q)
                                || s.lib.to_lowercase().contains(&q)
                        })
                        .take(60)
                        .map(|s| {
                            let url = s.url.clone();
                            let asset: Option<AssetRef> = s.asset.parse().ok();
                            widget(image_button(s.name.clone(), url))
                                .on_cx(move |_, _: &Activate, cx| {
                                    open.set(false);
                                    if let Some(asset) = asset.clone() {
                                        cx.dispatch_program_all(act(Tool::Insert(Inline::Atom(
                                            Atom {
                                                kind: AtomKind::Sticker,
                                                asset,
                                                height_em: 1.6,
                                                alt: String::new(),
                                                effects: Default::default(),
                                            },
                                        ))));
                                    }
                                })
                                .into_any()
                        })
                        .collect();
                    widget(Stack::row(6.0).wrap(true).width(LengthSpec::Fill))
                        .children(cells)
                        .into_any()
                },
            ),
        ))
        .into_any()
}

/// Input windows narrower than this fold their low-frequency tools into "…".
const COMPACT_BELOW: f32 = 900.0;

fn toolbar(ui: Ui) -> AnyView {
    let key = move || {
        (
            ui.locale.get(),
            ui.skin.with(|s| Arc::as_ptr(s) as usize),
            ui.settings
                .with(|s| (s.general.input_mode, s.assets.phrases.len())),
        )
    };
    dynamic(key, move |_| {
        let [
            color_open,
            size_open,
            ruby_open,
            fx_open,
            pace_open,
            insert_open,
            sticker_open,
            expr_open,
            phrase_open,
            stroke_open,
        ] = std::array::from_fn(|_| signal(false));
        let t = |k: &str| ui.now_t(k);
        let skin = ui.skin.get_untracked();
        let settings = ui.settings.get_untracked();
        let effects: Vec<AnyView> = [
            (ContinuousKind::Shake, "tb-effect-shake"),
            (ContinuousKind::Wave, "tb-effect-wave"),
            (ContinuousKind::Jump, "tb-effect-jump"),
            (ContinuousKind::Rainbow, "tb-effect-rainbow"),
        ]
        .into_iter()
        .map(|(kind, key)| item(t(key), fx_open, move || act(Tool::Effect(kind))))
        .chain([
            widget(Divider::horizontal()).into_any(),
            item(t("tb-entrance-fade"), fx_open, || {
                act(Tool::Entrance(EntranceKind::Fade))
            }),
            item(t("tb-entrance-pop"), fx_open, || {
                act(Tool::Entrance(EntranceKind::Pop))
            }),
            item(t("tb-entrance-slide"), fx_open, || {
                act(Tool::Entrance(EntranceKind::SlideUp))
            }),
        ])
        .collect();
        let pacing: Vec<AnyView> = [250u32, 500, 1000]
            .into_iter()
            .map(|ms| {
                let label = I18N.tr_in(
                    ui.locale.get_untracked(),
                    "tb-wait-ms",
                    &[("ms", ms.into())],
                );
                item(label, pace_open, move || marker(Marker::Wait { ms }))
            })
            .chain([
                widget(Divider::horizontal()).into_any(),
                item(t("tb-speed-slow"), pace_open, || {
                    act(Tool::Pace(Pace::Scale(2.0)))
                }),
                item(t("tb-speed-fast"), pace_open, || {
                    act(Tool::Pace(Pace::Scale(0.5)))
                }),
                item(t("tb-speed-instant"), pace_open, || {
                    act(Tool::Pace(Pace::Instant))
                }),
                item(t("tb-speed-reset"), pace_open, || {
                    act(Tool::Pace(Pace::Inherit))
                }),
                widget(Divider::horizontal()).into_any(),
                item(t("tb-pause"), pace_open, || {
                    marker(Marker::Pause { auto_ms: None })
                }),
                item(t("tb-pause-auto"), pace_open, || {
                    marker(Marker::Pause {
                        auto_ms: Some(1500),
                    })
                }),
                item(t("tb-page"), pace_open, || marker(Marker::Page)),
            ])
            .collect();
        let shake = skin.effects.screen_shake_px;
        let mut inserts: Vec<AnyView> = vec![
            item(t("tb-image"), insert_open, || {
                Msg::PickFile(FileTarget::InsertImage)
            }),
            item(t("tb-screenshake"), insert_open, move || {
                marker(Marker::ScreenShake {
                    amp: shake,
                    ms: 300,
                })
            }),
        ];
        let se: Vec<String> = skin.sounds.se.keys().cloned().collect();
        if !se.is_empty() {
            inserts.push(widget(Divider::horizontal()).into_any());
            for name in se {
                let label = format!("{} · {}", t("tb-sound"), name);
                inserts.push(item(label, insert_open, move || {
                    marker(Marker::Sound {
                        sound: name.clone(),
                        gain: 1.0,
                    })
                }));
            }
        }
        let stroke = |color: Rgba8| {
            act(Tool::Stroke(Some(Stroke {
                color,
                width_px: 2.0,
            })))
        };
        let accent = skin.chrome.accent;
        let strokes: Vec<AnyView> = vec![
            item(
                format!("{} · #ffffff", t("tb-stroke")),
                stroke_open,
                move || stroke(Rgba8::rgba(255, 255, 255, 255)),
            ),
            item(
                format!("{} · #000000", t("tb-stroke")),
                stroke_open,
                move || stroke(Rgba8::rgba(0, 0, 0, 255)),
            ),
            item(
                format!("{} · {}", t("tb-stroke"), accent),
                stroke_open,
                move || stroke(accent),
            ),
            item(t("tb-stroke-none"), stroke_open, || act(Tool::Stroke(None))),
            widget(Divider::horizontal()).into_any(),
            item(t("tb-shadow"), stroke_open, || {
                act(Tool::Shadow(Some(Shadow {
                    color: Rgba8::rgba(0, 0, 0, 0x99),
                    dx: 2.0,
                    dy: 2.0,
                    blur: 3.0,
                })))
            }),
            item(
                format!("{} · glow", t("tb-shadow")),
                stroke_open,
                move || {
                    act(Tool::Shadow(Some(Shadow {
                        color: accent.with_alpha(220),
                        dx: 0.0,
                        dy: 0.0,
                        blur: 8.0,
                    })))
                },
            ),
            item(t("tb-shadow-none"), stroke_open, || act(Tool::Shadow(None))),
        ];
        let phrases: Vec<AnyView> = if settings.assets.phrases.is_empty() {
            // The menu's empty state: ✦ and one line, then the way to add some.
            vec![
                widget(
                    ActionMenuItem::new(t("empty-phrases"))
                        .leading(ti::SPARKLES)
                        .disabled(true),
                )
                .into_any(),
                item(t("empty-open-assets"), phrase_open, || {
                    Msg::SettingsPage(super::settings::Page::Assets)
                }),
            ]
        } else {
            settings
                .assets
                .phrases
                .iter()
                .enumerate()
                .map(|(i, p)| item(p.label.clone(), phrase_open, move || Msg::InsertPhrase(i)))
                .collect()
        };
        let mode = settings.general.input_mode;
        let mode_switch = segmented().children((
            segmented_option(t("mode-compose-short"))
                .selected(mode == InputMode::Compose)
                .on_cx(|_, _: &nana_ui::runtime::SegmentedSelectionRequested, cx| {
                    cx.dispatch_program_all(Msg::Edit(Box::new(|s: &mut Settings| {
                        s.general.input_mode = InputMode::Compose
                    })))
                }),
            segmented_option(t("mode-mirror-short"))
                .selected(mode == InputMode::Mirror)
                .on_cx(|_, _: &nana_ui::runtime::SegmentedSelectionRequested, cx| {
                    cx.dispatch_program_all(Msg::Edit(Box::new(|s: &mut Settings| {
                        s.general.input_mode = InputMode::Mirror
                    })))
                }),
        ));
        // Below this width the low-frequency tools fold into a "…" menu so
        // Send and the mode switch always stay on screen.
        let compact =
            crate::windows::size_of(WindowKind::Input, &skin, &settings).0 < COMPACT_BELOW;
        let [more_open] = std::array::from_fn(|_| signal(false));
        let open_item = |label: String, target: Signal<bool>| -> AnyView {
            widget(ActionMenuItem::new(label))
                .on_cx(move |_, _: &Activate, _| {
                    more_open.set(false);
                    target.set(true);
                })
                .into_any()
        };
        let mut bar: Vec<AnyView> = vec![
            toggle(ui, ti::BOLD, "tb-bold", || Tool::Bold, is_bold),
            toggle(ui, ti::ITALIC, "tb-italic", || Tool::Italic, is_italic),
            toggle(
                ui,
                ti::UNDERLINE,
                "tb-underline",
                || Tool::Underline,
                is_underline,
            ),
            toggle(
                ui,
                ti::STRIKETHROUGH,
                "tb-strike",
                || Tool::Strike,
                is_strike,
            ),
            popover(
                ti::PALETTE,
                t("tb-color"),
                220.0,
                color_open,
                color_panel(ui, color_open),
            )
            .into_any(),
            popover(
                ti::TEXT_SIZE,
                t("tb-size"),
                240.0,
                size_open,
                size_panel(ui, size_open),
            )
            .into_any(),
        ];
        let ruby = popover(
            ti::LANGUAGE_HIRAGANA,
            t("tb-ruby"),
            240.0,
            ruby_open,
            ruby_panel(ui, ruby_open),
        );
        let sticker = popover(
            ti::STICKER,
            t("tb-sticker"),
            360.0,
            sticker_open,
            sticker_panel(ui, sticker_open),
        );
        let expression = popover(
            ti::MOOD_SMILE,
            t("tb-expression"),
            260.0,
            expr_open,
            expression_panel(ui, expr_open),
        );
        let divider = || widget(Divider::vertical().inset(6.0)).into_any();
        let effects_menu = menu(ti::SPARKLES, t("tb-effects"), fx_open, effects);
        let pacing_menu = menu(ti::HOURGLASS, t("tb-pacing"), pace_open, pacing);
        let clear_format = tool(ui, ti::CLEAR_FORMATTING, "tb-clear-format", || {
            act(Tool::ClearFormat)
        });
        // The popovers of folded tools stay mounted, invisible and out of the
        // flow, so the "…" menu can open them in place.
        let mut hidden: Vec<AnyView> = Vec::new();
        if compact {
            bar.extend([divider(), effects_menu, pacing_menu, clear_format]);
            let mut items: Vec<AnyView> = strokes;
            items.push(widget(Divider::horizontal()).into_any());
            items.push(open_item(t("tb-ruby"), ruby_open));
            items.push(open_item(t("tb-sticker"), sticker_open));
            items.push(open_item(t("tb-expression"), expr_open));
            items.push(
                widget(ActionMenuItem::new(t("tb-source")))
                    .on_cx(move |_, _: &Activate, cx| {
                        more_open.set(false);
                        cx.dispatch_program_all(Msg::ToggleSource);
                    })
                    .into_any(),
            );
            items.push(widget(Divider::horizontal()).into_any());
            items.extend(inserts);
            bar.push(menu(ti::DOTS, t("tb-more"), more_open, items));
            hidden.extend([ruby, sticker, expression]);
        } else {
            bar.push(menu(ti::BORDER_OUTER, t("tb-stroke"), stroke_open, strokes));
            bar.push(ruby);
            bar.push(divider());
            bar.extend([effects_menu, pacing_menu, sticker, expression]);
            bar.push(menu(ti::TEXT_PLUS, t("tb-insert"), insert_open, inserts));
            bar.push(clear_format);
            bar.push(source_toggle(ui));
        }
        bar.push(widget(Stack::spacer()).into_any());
        if !hidden.is_empty() {
            bar.push(
                widget(Stack::row(0.0).with_layout(|l| {
                    l.width = Some(LengthSpec::Px(0.0));
                    l.flex_shrink = Some(0.0);
                    l.opacity = Some(0.0);
                    l.pointer_events = Some(PointerEventsSpec::None);
                }))
                .children(hidden)
                .into_any(),
            );
        }
        // Pinned right: never shrinks, so Send is not clipped.
        let mut pinned: Vec<AnyView> = vec![
            mode_switch.into_any(),
            menu(ti::MESSAGE, t("tb-phrases"), phrase_open, phrases),
            tool(ui, ti::SETTINGS, "window-settings", || Msg::OpenSettings),
        ];
        if mode == InputMode::Compose {
            let key = match settings.general.send_key {
                SendKey::Enter => "Enter",
                SendKey::CtrlEnter => "Ctrl+Enter",
            };
            pinned.push(keycap(key));
        }
        pinned.push(
            widget(
                Button::new(t("tb-send"))
                    .icon(ti::SEND)
                    .kind(ButtonKind::Primary)
                    .size(ControlSize::Small),
            )
            .on_cx(|_, _: &Activate, cx| cx.dispatch_program_all(Msg::Core(Cmd::Send)))
            .into_any(),
        );
        bar.push(
            widget(Stack::row(2.0).with_layout(|l| {
                l.flex_shrink = Some(0.0);
                l.flex_grow = Some(0.0);
                l.align_items = AlignSpec::Center;
            }))
            .children(pinned)
            .into_any(),
        );
        widget(
            Stack::bar(2.0)
                .height(LengthSpec::Px(TOOLBAR_H))
                .padding_xy(8.0, 6.0)
                .align(AlignSpec::Center),
        )
        .children(bar)
        .into_any()
    })
    .into_any()
}

/// Nana pink, for live states (mirroring).
const PINK: [f32; 4] = [
    0xe8 as f32 / 255.0,
    0x6a as f32 / 255.0,
    0x92 as f32 / 255.0,
    1.0,
];

/// Period of a breathing status pulse (ms).
pub const PULSE_MS: u64 = 1200;

/// Whether the input window needs frames for the mirroring chip's pulse.
pub fn mirror_chip_pulsing(ui: Ui) -> bool {
    !ui.reduced_motion.get_untracked()
        && ui
            .settings
            .with_untracked(|s| s.general.input_mode == InputMode::Mirror)
}

/// The send key as a keycap beside the Send button.
fn keycap(key: &str) -> AnyView {
    chip(key.to_owned(), [0.5, 0.5, 0.5, 1.0], None, 11.0, 4.0)
}

/// `text` in a small label: a filled pill when `fill` is given, an outlined
/// keycap otherwise.
fn chip(text: String, color: [f32; 4], fill: Option<[f32; 4]>, size: f32, radius: f32) -> AnyView {
    let mut st = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.font_size = Some(size);
        l.color = Some(color);
        l.white_space_nowrap = true;
    }
    widget(Stack::row(0.0).with_layout(move |l| {
        l.flex_shrink = Some(0.0);
        l.padding_left = Some(LengthSpec::Px(7.0));
        l.padding_right = Some(LengthSpec::Px(7.0));
        l.padding_top = Some(LengthSpec::Px(1.0));
        l.padding_bottom = Some(LengthSpec::Px(1.0));
        l.border_radius = Some(radius);
        l.border_width = Some(1.0);
        l.border_style = Some(nana_ui_core::BorderStyle::Solid);
        match fill {
            Some(f) => {
                l.background = Some(f);
                l.border_color = Some(f);
            }
            None => l.border_color = Some([color[0], color[1], color[2], 0.55]),
        }
    }))
    .children(widget(Text::new(text).decorative().style(st)))
    .into_any()
}

fn status_line(ui: Ui) -> AnyView {
    enum Line {
        Error(String),
        Mirror(String),
        Queued(String),
        Empty,
    }
    // Notices show as toasts above this line.
    let key = move || {
        let status = ui.status.get();
        let diag = ui.diagnostics.with(|d| d.first().cloned());
        let (mode, reduced) = (
            ui.settings.with(|s| s.general.input_mode),
            ui.reduced_motion.get(),
        );
        (diag, status.queued, mode, reduced, ui.locale.get())
    };
    let line = dynamic(key, move |k| {
        let (diag, queued, mode, reduced, locale) = k.clone();
        let line = if let Some(d) = diag {
            Line::Error(d)
        } else if queued > 0 {
            Line::Queued(I18N.tr_in(locale, "status-queued-chip", &[("position", queued.into())]))
        } else if mode == InputMode::Mirror {
            Line::Mirror(I18N.t_in(locale, "status-mirror"))
        } else {
            Line::Empty
        };
        match line {
            Line::Error(text) => {
                let mut st = NodeStyle::default();
                Arc::make_mut(&mut st.layout).font_size = Some(12.0);
                widget(
                    Text::new(text)
                        .style(st)
                        .truncating()
                        .color(SemanticColorRole::Danger),
                )
                .into_any()
            }
            Line::Queued(text) => {
                let accent = c(ui.skin.get_untracked().chrome.accent);
                chip(
                    text,
                    accent,
                    Some([accent[0], accent[1], accent[2], 0.16]),
                    12.0,
                    10.0,
                )
            }
            Line::Mirror(text) => {
                let fill = [PINK[0], PINK[1], PINK[2], 0.16];
                let mut chip_view =
                    widget(Stack::row(0.0).with_layout(|l| l.flex_shrink = Some(0.0)))
                        .children(chip(format!("● {text}"), PINK, Some(fill), 12.0, 10.0));
                if !reduced {
                    // Opacity breathes once per PULSE_MS.
                    chip_view = chip_view.bind(move |s: &mut Stack| {
                        let phase = (ui.pulse_ms.get() % PULSE_MS) as f32 / PULSE_MS as f32;
                        let wave = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
                        restyle(s, |l| l.opacity = Some(0.55 + 0.45 * wave));
                    });
                }
                chip_view.into_any()
            }
            Line::Empty => widget(Stack::row(0.0)).into_any(),
        }
    });
    widget(
        Stack::bar(6.0)
            .height(LengthSpec::Px(STATUS_H))
            .padding_xy(12.0, 2.0),
    )
    .children((
        widget(Stack::row(0.0).with_layout(|l| {
            l.flex_grow = Some(1.0);
            l.min_width = Some(LengthSpec::Px(0.0));
        }))
        .visible(move || ui.reply.with(Option::is_none))
        .children(line),
        reply_bar(ui),
    ))
    .into_any()
}

/// "Replying to": the quoted chat message the next line answers, with a
/// button that drops it.
fn reply_bar(ui: Ui) -> AnyView {
    dynamic(
        move || ui.reply.get(),
        move |reply| {
            let Some(reply) = reply else {
                return widget(Stack::row(0.0)).into_any();
            };
            let skin = ui.skin.get_untracked();
            let url = reply.user.avatar.as_ref().and_then(|a| {
                ui.asset_urls
                    .with_untracked(|u| u.get(&a.to_string()).cloned())
            });
            let mut label = NodeStyle::default();
            {
                let l = Arc::make_mut(&mut label.layout);
                l.font_size = Some(12.0);
                l.font_weight = Some(700);
                l.color = Some(super::c(skin.chrome.accent));
                l.white_space_nowrap = true;
                l.flex_shrink = Some(0.0);
            }
            let mut body = NodeStyle::default();
            Arc::make_mut(&mut body.layout).font_size = Some(12.0);
            body.foreground = Some(SemanticColorRole::Muted);
            widget(
                Stack::row(6.0)
                    .align(AlignSpec::Center)
                    .width(LengthSpec::Fill),
            )
            .children((
                widget(
                    Text::new(I18N.tr_in(
                        ui.locale.get_untracked(),
                        "chat-replying",
                        &[("name", reply.user.name.as_str().into())],
                    ))
                    .style(label),
                ),
                super::danmaku::avatar(url, &reply.user.name, 18.0, super::c(skin.chrome.accent)),
                widget(Stack::row(0.0).with_layout(|l| {
                    l.flex_grow = Some(1.0);
                    l.flex_shrink = Some(1.0);
                    l.min_width = Some(LengthSpec::Px(0.0));
                }))
                .children(widget(
                    Text::new(reply.text.clone()).style(body).truncating(),
                )),
                widget(
                    IconButton::new(ti::X, ui.now_t("chat-reply-cancel"))
                        .size(ControlSize::Small)
                        .with_tooltip(ui.now_t("chat-reply-cancel")),
                )
                .on_cx(|_, _: &Activate, cx| cx.dispatch_program_all(Msg::ClearReply)),
            ))
            .into_any()
        },
    )
    .into_any()
}

/// The draft inside the dialogue box: the rich editor, or the markup source.
/// Both are built once and shown by `source_mode`, so the editor keeps its
/// selection and undo history across toggles.
fn editor_area(ui: Ui, editor: EntityRef<RichTextEditor>, source: EntityRef<TextArea>) -> AnyView {
    let style = move || {
        let mut st = Base::editor(&ui.skin.get()).node_style();
        let l = Arc::make_mut(&mut st.layout);
        l.width = Some(LengthSpec::Fill);
        l.flex_grow = Some(1.0);
        // Held to the text area, so a long draft scrolls inside the editor
        // (which keeps the caret in view) instead of running off the box.
        l.flex_shrink = Some(1.0);
        l.min_height = Some(LengthSpec::Px(0.0));
        l.cursor = Some(nana_ui_core::CursorSpec::Text);
        st
    };
    let placeholder_style = move || {
        let mut st = Base::editor(&ui.skin.get()).node_style();
        let l = Arc::make_mut(&mut st.layout);
        l.position = PositionSpec::Absolute;
        l.offset_left = Some(LengthSpec::Px(0.0));
        l.offset_top = Some(LengthSpec::Px(0.0));
        l.opacity = Some(0.45);
        l.pointer_events = Some(PointerEventsSpec::None);
        st
    };
    widget(Stack::fill_column(0.0).with_layout(|l| {
        l.position = PositionSpec::Relative;
        l.flex_grow = Some(1.0);
    }))
    .children((
        widget(Stack::fill_column(0.0).with_layout(|l| l.flex_grow = Some(1.0)))
            .visible(move || !ui.source_mode.get())
            .children((
                widget(RichTextEditor::new(ui.editor_value.get_untracked()))
                    .entity_ref(editor)
                    .bind(move |e: &mut RichTextEditor| {
                        ui.editor_value.with(|v| {
                            if e.value != *v {
                                e.value = v.clone();
                            }
                        });
                        let st = style();
                        if e.style != st {
                            e.style = st;
                        }
                    })
                    .on_cx(move |_, e: &RichTextEditorEvent, cx| match e {
                        RichTextEditorEvent::Changed(rich) => {
                            cx.dispatch_program_all(Msg::EditorChanged(rich.clone()))
                        }
                        RichTextEditorEvent::SelectionChanged { attrs, .. } => {
                            if ui.sel_attrs.with_untracked(|a| a != attrs) {
                                ui.sel_attrs.set(attrs.clone());
                            }
                        }
                        RichTextEditorEvent::PasteRequested => {
                            cx.dispatch_program_all(Msg::EditorPaste)
                        }
                        _ => {}
                    }),
                widget(Text::new(ui.now_t("editor-placeholder")))
                    .bind(move |t: &mut Text| {
                        let st = placeholder_style();
                        if t.style != st {
                            t.style = st;
                        }
                        let value = I18N.t_in(ui.locale.get(), "editor-placeholder");
                        if t.value != value {
                            t.value = value;
                        }
                    })
                    .visible(move || ui.draft.with(|d| d.is_blank())),
            )),
        widget(Stack::fill_column(0.0).with_layout(|l| l.flex_grow = Some(1.0)))
            .visible(move || ui.source_mode.get())
            .children(
                widget({
                    let mut area =
                        TextArea::new("").placeholder(ui.now_t("editor-source-placeholder"));
                    let l = Arc::make_mut(&mut area.style.layout);
                    l.width = Some(LengthSpec::Fill);
                    l.height = Some(LengthSpec::Percent(100.0));
                    area
                })
                .entity_ref(source)
                .bind(move |a: &mut TextArea| {
                    let text = ui.source.with(|s| s.text.clone());
                    if a.state.value.as_str() != text {
                        a.state.replace_value(text);
                    }
                })
                .on_cx(move |_, e: &TextChanged, cx| {
                    let range = e.selection.range();
                    cx.dispatch_program_all(Msg::Markup(MarkupEdit::Typed {
                        text: e.value.to_string(),
                        selection: (range.start, range.end),
                    }));
                }),
            ),
    ))
    .into_any()
}

/// The input window. Returns the editor and the source field for key
/// handling, and the stack the host shows toasts in.
pub fn input_window(ui: Ui) -> WithRefs<impl IntoView, (EditorRef, SourceRef, ToastsRef)> {
    let editor = entity_ref::<RichTextEditor>();
    let source = entity_ref::<TextArea>();
    let toasts = entity_ref::<nana_ui::runtime::ToastStack>();
    let region_h = move || {
        let skin = ui.skin.get();
        let s = ui.settings.get();
        let region = s.skin.overrides.region.unwrap_or(skin.dialogue.region);
        region[1] + skin.dialogue.overscan * 2.0
    };
    let view = widget(
        Stack::fill_column(0.0)
            .surface(SemanticColorRole::Background)
            .radius(nana_ui_core::RadiusTier::Lg)
            .with_layout(|l| {
                l.position = PositionSpec::Relative;
                l.overflow_x = nana_ui_core::OverflowSpec::Hidden;
                l.overflow_y = nana_ui_core::OverflowSpec::Hidden;
            }),
    )
    .children((
        toolbar(ui),
        widget(Stack::column(0.0))
            .bind(move |s: &mut Stack| {
                let h = region_h();
                let mut taken = std::mem::replace(s, Stack::row(0.0));
                taken = taken.height(LengthSpec::Px(h));
                *s = taken;
            })
            .children(dialogue_box(
                ui,
                Source::Draft,
                Some(editor_area(ui, editor, source)),
            )),
        status_line(ui),
        super::feedback::toast_stack(toasts, STATUS_H + 4.0, 2),
    ));
    with_refs(view, (editor, source, toasts))
}

pub type EditorRef = EntityRef<RichTextEditor>;
pub type SourceRef = EntityRef<TextArea>;
pub type ToastsRef = EntityRef<nana_ui::runtime::ToastStack>;
