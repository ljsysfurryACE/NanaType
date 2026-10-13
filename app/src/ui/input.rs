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
    ActionMenu, ActionMenuItem, Activate, Button, ColorChanged, ColorField, Divider,
    FocusWithinChanged, IconButton, IconGlyph, NodeStyle, Popover, PopoverClosed, PopoverToggled,
    RangeField, RangeInput, RichAttrMask, RichSelectionAttrs, RichTextEditor, RichTextEditorEvent,
    Stack, Text, TextArea, TextChanged, TextInput, TextSubmitted,
};
use nana_ui::theme::space;
use nana_ui::{ButtonKind, ControlSize, PopoverPlacement};
use nana_ui_core::{
    AlignSpec, JustifySpec, LengthSpec, OverflowSpec, PointerEventsSpec, PositionSpec,
    SemanticColorRole, SquareSize,
};
use nanatype_core::Cmd;
use nanatype_i18n::{Arg, Locale};
use nanatype_model::{
    AssetRef, Atom, AtomKind, ContinuousKind, EntranceKind, Inline, Marker, Pace, Rgba8, Shadow,
    Stroke, VtsAction,
};
use nanatype_settings::{ChromeMode, InputMode, SendKey, Settings};

use super::dialogue::{Base, Source, dialogue_box};
use super::rich::{ChipKind, MarkerLook};
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

/// Toolbar glyphs: one size for every tool, centred in a 28px target.
const GLYPH: f32 = 14.0;
/// Icon-button hit target.
const TARGET: f32 = 28.0;

/// The toolbar's resting ink: the chrome's text at 75%, while the window
/// wears the skin's own chrome. `None` (another light or dark mode is
/// forced) leaves the theme's muted ink.
fn tool_ink(ui: Ui) -> Option<[f32; 4]> {
    let skin = ui.skin.get_untracked();
    let chrome = &skin.chrome;
    let dark = chrome.appearance == nanatype_theme::Appearance::Dark;
    let own = match ui.settings.with_untracked(|s| s.skin.chrome) {
        ChromeMode::Skin => true,
        ChromeMode::Light => !dark,
        ChromeMode::Dark => dark,
        ChromeMode::System => false,
    };
    own.then(|| {
        let text = chrome.palette.get("text").copied().unwrap_or(chrome.text);
        let [r, g, b, _] = c(text);
        [r, g, b, 0.75]
    })
}

/// A toolbar icon button: the shared glyph size in a 28px square, in the
/// toolbar ink.
fn tool_button(
    icon: nana_ui::runtime::Icon,
    label: String,
    tip: String,
    ink: Option<[f32; 4]>,
) -> IconButton {
    // Large is the size whose glyph is GLYPH; the square stays 28px.
    let mut b = IconButton::new(icon, label)
        .size(ControlSize::Large)
        .with_tooltip(tip);
    b.style.square = Some(SquareSize::IconButton);
    if let Some(ink) = ink {
        b = b.colors_from_style();
        paint_tool(&mut b.style, ink, false);
    }
    b
}

/// Rest a toolbar button in `ink`, or in the soft accent while pressed.
fn paint_tool(style: &mut NodeStyle, ink: [f32; 4], selected: bool) {
    if selected {
        style.background = Some(SemanticColorRole::AccentSoft);
        style.foreground = Some(SemanticColorRole::AccentOnSoft);
        Arc::make_mut(&mut style.layout).color = None;
    } else {
        style.background = None;
        style.foreground = None;
        Arc::make_mut(&mut style.layout).color = Some(ink);
    }
}

/// "Bold · Ctrl+B": a tooltip with the tool's shortcut.
fn with_shortcut(label: String, keys: Option<&str>) -> String {
    match keys {
        Some(k) => format!("{label} · {k}"),
        None => label,
    }
}

/// An icon button with a tooltip that sends `msg()`.
fn tool(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    label: &'static str,
    msg: impl Fn() -> Msg + Send + 'static,
) -> AnyView {
    widget(tool_button(
        icon,
        ui.now_t(label),
        ui.now_t(label),
        tool_ink(ui),
    ))
    .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(msg()))
    .into_any()
}

/// A toolbar toggle (bold, italic…) shown pressed while the whole selection
/// has the attribute; its tooltip names its shortcut.
fn toggle(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    label: &'static str,
    keys: Option<&'static str>,
    tool_of: fn() -> Tool,
    pressed: fn(&RichSelectionAttrs) -> bool,
) -> AnyView {
    let ink = tool_ink(ui);
    widget(tool_button(
        icon,
        ui.now_t(label),
        with_shortcut(ui.now_t(label), keys),
        ink,
    ))
    .bind(move |b: &mut IconButton| {
        let on = !ui.source_mode.get() && ui.sel_attrs.with(pressed);
        if b.selected != on {
            b.selected = on;
            if let Some(ink) = ink {
                paint_tool(&mut b.style, ink, on);
            }
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
    let ink = tool_ink(ui);
    widget(tool_button(
        ti::CODE,
        ui.now_t("tb-source"),
        ui.now_t("tb-source"),
        ink,
    ))
    .bind(move |b: &mut IconButton| {
        let on = ui.source_mode.get();
        if b.selected != on {
            b.selected = on;
            if let Some(ink) = ink {
                paint_tool(&mut b.style, ink, on);
            }
        }
    })
    .on_cx(|_, _: &Activate, cx| cx.dispatch_program_all(Msg::ToggleSource))
    .into_any()
}

/// A menu trigger's face. A bare glyph is centred in the box left after the
/// trigger's own `space::LG` padding, so the hit target stays [`TARGET`].
fn trigger_face(
    icon: nana_ui::runtime::Icon,
    label: Option<String>,
    ink: Option<[f32; 4]>,
) -> AnyView {
    let mut glyph = IconGlyph::new(icon).size(GLYPH).decorative();
    if let Some(ink) = ink {
        glyph.style.foreground = None;
        Arc::make_mut(&mut glyph.style.layout).color = Some(ink);
    }
    match label {
        Some(label) => {
            let mut st = text_style(12.0, 500);
            {
                let l = Arc::make_mut(&mut st.layout);
                l.white_space_nowrap = true;
                l.color = ink;
            }
            let mut text = Text::new(label).decorative().style(st);
            if ink.is_none() {
                text = text.color(SemanticColorRole::Muted);
            }
            widget(Stack::row(space::XS).align(AlignSpec::Center))
                .children((widget(glyph), widget(text)))
                .into_any()
        }
        None => {
            let inner = TARGET - 2.0 * space::LG;
            widget(
                Stack::row(0.0)
                    .align(AlignSpec::Center)
                    .justify(JustifySpec::Center)
                    .with_layout(|l| {
                        l.width = Some(LengthSpec::Px(inner));
                        l.min_width = Some(LengthSpec::Px(inner));
                        l.height = Some(LengthSpec::Px(GLYPH));
                        l.flex_shrink = Some(0.0);
                        l.overflow_x = OverflowSpec::Visible;
                        l.overflow_y = OverflowSpec::Visible;
                    }),
            )
            .children(widget(glyph))
            .into_any()
        }
    }
}

fn text_style(size: f32, weight: u16) -> NodeStyle {
    let mut st = NodeStyle::default();
    let l = Arc::make_mut(&mut st.layout);
    l.font_size = Some(size);
    l.font_weight = Some(weight);
    st
}

/// A popover whose open state the app keeps, so items can close it.
fn popover(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    label: String,
    width: f32,
    open: Signal<bool>,
    body: impl IntoView,
) -> AnyView {
    widget(
        Popover::new()
            .trigger(label)
            .placement(PopoverPlacement::Bottom)
            .width(width)
            .bare_trigger(true),
    )
    .trigger(trigger_face(icon, None, tool_ink(ui)))
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

/// An action menu behind a toolbar glyph, labelled beside the glyph when
/// `labelled` (the menus whose glyph alone does not say what they hold).
fn menu(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    label: String,
    labelled: bool,
    open: Signal<bool>,
    items: Vec<AnyView>,
) -> AnyView {
    let face = trigger_face(icon, labelled.then(|| label.clone()), tool_ink(ui));
    widget(ActionMenu::new().trigger(label).bare_trigger(true))
        .trigger(face)
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
        l.border_width = Some(1.0);
        l.border_style = Some(nana_ui_core::BorderStyle::Solid);
    }
    st.radius = Some(nana_ui_core::RadiusTier::Xs);
    st.border = Some(SemanticColorRole::BorderStrong);
    {
        let l = Arc::make_mut(&mut st.layout);
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
        l.paint.background_image = Some(nana_ui_core::BackgroundImage::url_with_fit(
            url,
            nana_ui_core::BackgroundImageFit::Contain,
        ));
    }
    st.radius = Some(nana_ui_core::RadiusTier::Sm);
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

/// The toolbar's sticker picker; `open` is its popover's state.
pub fn sticker_panel(ui: Ui, open: Signal<bool>) -> AnyView {
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

/// A small caption over a group in a toolbar panel.
fn caption(text: String) -> AnyView {
    widget(
        Text::new(text)
            .style(text_style(11.0, 600))
            .color(SemanticColorRole::Muted),
    )
    .into_any()
}

/// Outline and shadow: colour swatches for the outline, named looks for the
/// shadow, each with a way to take it off.
fn stroke_panel(ui: Ui, open: Signal<bool>) -> AnyView {
    let accent = ui.skin.get_untracked().chrome.accent;
    let stroke = move |color: Rgba8| {
        Tool::Stroke(Some(Stroke {
            color,
            width_px: 2.0,
        }))
    };
    // A button that applies `tool` and closes the panel.
    let press = move |button: Button, tool: Tool| -> AnyView {
        widget(button)
            .on_cx(move |_, _: &Activate, cx| {
                open.set(false);
                cx.dispatch_program_all(act(tool.clone()));
            })
            .into_any()
    };
    let swatches: Vec<AnyView> = [
        ("tb-stroke-white", Rgba8::rgba(255, 255, 255, 255)),
        ("tb-stroke-black", Rgba8::rgba(0, 0, 0, 255)),
        ("tb-stroke-accent", accent),
    ]
    .into_iter()
    .map(|(key, color)| press(swatch_button(ui.now_t(key), c(color)), stroke(color)))
    .collect();
    let small = |key: &str| Button::new(ui.now_t(key)).size(ControlSize::Small);
    let drop = Shadow {
        color: Rgba8::rgba(0, 0, 0, 0x99),
        dx: 2.0,
        dy: 2.0,
        blur: 3.0,
    };
    let glow = Shadow {
        color: accent.with_alpha(220),
        dx: 0.0,
        dy: 0.0,
        blur: 8.0,
    };
    widget(Stack::column(8.0).padding(4.0))
        .children((
            caption(ui.now_t("tb-stroke")),
            widget(Stack::row(6.0).align(AlignSpec::Center))
                .children((swatches, press(small("tb-stroke-none"), Tool::Stroke(None)))),
            caption(ui.now_t("tb-shadow")),
            widget(Stack::row(6.0).align(AlignSpec::Center)).children((
                press(small("tb-shadow-drop"), Tool::Shadow(Some(drop))),
                press(
                    small("tb-shadow-glow").icon(ti::SPARKLES),
                    Tool::Shadow(Some(glow)),
                ),
                press(small("tb-shadow-none"), Tool::Shadow(None)),
            )),
        ))
        .into_any()
}

/// A marker as the editor shows it, for the cheat sheet: the kind's stripe
/// and glyph, then its markup, on the editor's chip fill.
fn marker_chip(
    look: &MarkerLook,
    kind: ChipKind,
    icon: nana_ui::runtime::Icon,
    markup: &str,
) -> AnyView {
    let ink = c(look.text);
    let mut glyph = IconGlyph::new(icon).size(13.0).decorative();
    glyph.style.foreground = None;
    Arc::make_mut(&mut glyph.style.layout).color = Some(ink);
    let mut st = text_style(12.0, 600);
    {
        let l = Arc::make_mut(&mut st.layout);
        l.color = Some(ink);
        l.white_space_nowrap = true;
    }
    let fill = c(look.fill);
    let hue = c(kind.hue());
    widget(
        Stack::row(5.0)
            .align(AlignSpec::Center)
            .with_layout(move |l| {
                l.height = Some(LengthSpec::Px(24.0));
                l.flex_shrink = Some(0.0);
                l.background = Some(fill);
                l.border_radius = Some(4.0);
                l.padding_right = Some(LengthSpec::Px(8.0));
                l.overflow_x = nana_ui_core::OverflowSpec::Hidden;
                l.overflow_y = nana_ui_core::OverflowSpec::Hidden;
            }),
    )
    .children((
        widget(Stack::row(0.0).with_layout(move |l| {
            l.width = Some(LengthSpec::Px(3.0));
            l.height = Some(LengthSpec::Px(24.0));
            l.flex_shrink = Some(0.0);
            l.background = Some(hue);
        })),
        widget(glyph),
        widget(Text::new(markup).decorative().style(st)),
    ))
    .into_any()
}

/// One line of the cheat sheet: a fixed-width lead (a chip or keycaps) and
/// what it does.
fn sheet_row(lead: AnyView, lead_w: f32, meaning: String) -> AnyView {
    let mut st = text_style(12.0, 400);
    Arc::make_mut(&mut st.layout).white_space_nowrap = true;
    widget(Stack::row(10.0).align(AlignSpec::Center).with_layout(|l| {
        l.height = Some(LengthSpec::Px(24.0));
    }))
    .children((
        widget(
            Stack::row(0.0)
                .align(AlignSpec::Center)
                .with_layout(move |l| {
                    l.width = Some(LengthSpec::Px(lead_w));
                    l.min_width = Some(LengthSpec::Px(lead_w));
                    l.flex_shrink = Some(0.0);
                }),
        )
        .children(lead),
        widget(Text::new(meaning).style(st).color(SemanticColorRole::Muted)),
    ))
    .into_any()
}

/// The `?` sheet: the markers the editor shows as chips with what each does,
/// and the input window's own keys.
pub fn cheat_sheet(ui: Ui) -> AnyView {
    let skin = ui.skin.get_untracked();
    let look = MarkerLook::from_skin(&skin);
    let t = |k: &str| ui.now_t(k);
    let markers: Vec<AnyView> = [
        (ChipKind::Wait, ti::CLOCK, "[wait=500]", "cheat-wait"),
        (
            ChipKind::Continue,
            ti::PLAYER_PLAY,
            "[pause]",
            "cheat-pause",
        ),
        (ChipKind::Page, ti::PAGE_BREAK, "[page]", "cheat-page"),
        (
            ChipKind::Expression,
            ti::MOOD_SMILE,
            "[expr=…]",
            "cheat-expr",
        ),
        (ChipKind::Sound, ti::MUSIC, "[se=…]", "cheat-se"),
        (
            ChipKind::Picture,
            ti::STICKER,
            "[sticker=…]",
            "cheat-sticker",
        ),
        (ChipKind::Mention, ti::AT, "[at=…]", "cheat-at"),
    ]
    .into_iter()
    .map(|(kind, icon, markup, key)| {
        sheet_row(marker_chip(&look, kind, icon, markup), 116.0, t(key))
    })
    .collect();
    let (send, newline) = match ui.settings.with_untracked(|s| s.general.send_key) {
        SendKey::Enter => ("Enter", "Shift+Enter"),
        SendKey::CtrlEnter => ("Ctrl+Enter", "Enter"),
    };
    let recall = widget(Stack::row(4.0).align(AlignSpec::Center))
        .children((
            super::keycap::chord("Ctrl+↑"),
            widget(
                Text::new("/")
                    .decorative()
                    .style(text_style(12.0, 500))
                    .color(SemanticColorRole::Muted),
            ),
            super::keycap::keycap("↓"),
        ))
        .into_any();
    let keys: Vec<AnyView> = vec![
        sheet_row(super::keycap::chord(send), 136.0, t("tb-send")),
        sheet_row(super::keycap::chord(newline), 136.0, t("cheat-newline")),
        sheet_row(super::keycap::chord("Ctrl+B"), 136.0, t("tb-bold")),
        sheet_row(super::keycap::chord("Ctrl+I"), 136.0, t("tb-italic")),
        sheet_row(super::keycap::chord("Ctrl+U"), 136.0, t("tb-underline")),
        sheet_row(recall, 136.0, t("cheat-recall")),
        sheet_row(super::keycap::chord("Esc"), 136.0, t("cheat-hide")),
    ];
    let column = |title: String, rows: Vec<AnyView>| {
        widget(Stack::column(4.0).width(LengthSpec::Shrink))
            .children((caption(title), rows))
            .into_any()
    };
    widget(Stack::row(16.0).padding(4.0))
        .children((
            column(t("cheat-markers"), markers),
            column(t("cheat-shortcuts"), keys),
        ))
        .into_any()
}

/// A hairline between two groups of toolbar tools, 16px tall.
fn group_divider() -> AnyView {
    widget(
        Stack::row(0.0)
            .surface(SemanticColorRole::Border)
            .with_layout(|l| {
                l.width = Some(LengthSpec::Px(1.0));
                l.min_width = Some(LengthSpec::Px(1.0));
                l.height = Some(LengthSpec::Px(16.0));
                l.flex_shrink = Some(0.0);
                l.margin_left = Some(LengthSpec::Px(5.0));
                l.margin_right = Some(LengthSpec::Px(5.0));
            }),
    )
    .into_any()
}

/// A 6×28 grip at the toolbar's left edge: two columns of dots that say the
/// window moves by dragging when the system frame is hidden. It takes no
/// pointer events, so a press on it falls through to the toolbar's empty
/// area, which the host turns into a window drag. Only the input window has
/// it; the broadcast windows stay clean for capture.
fn drag_grip() -> AnyView {
    let dot = || {
        widget(
            Stack::row(0.0)
                .surface(SemanticColorRole::Muted)
                .with_layout(|l| {
                    l.width = Some(LengthSpec::Px(2.0));
                    l.height = Some(LengthSpec::Px(2.0));
                    l.min_width = Some(LengthSpec::Px(2.0));
                    l.flex_shrink = Some(0.0);
                    l.border_radius = Some(1.0);
                }),
        )
        .into_any()
    };
    let rows: Vec<AnyView> = (0..4)
        .map(|_| widget(Stack::row(2.0)).children((dot(), dot())).into_any())
        .collect();
    widget(
        Stack::column(3.0)
            .justify(nana_ui_core::JustifySpec::Center)
            .with_layout(|l| {
                l.width = Some(LengthSpec::Px(6.0));
                l.min_width = Some(LengthSpec::Px(6.0));
                l.height = Some(LengthSpec::Px(28.0));
                l.flex_shrink = Some(0.0);
                l.margin_right = Some(LengthSpec::Px(6.0));
                l.opacity = Some(0.6);
                l.pointer_events = Some(PointerEventsSpec::None);
            }),
    )
    .children(rows)
    .into_any()
}

/// Input windows narrower than this fold their low-frequency tools into "…".
const COMPACT_BELOW: f32 = 900.0;

fn toolbar(ui: Ui) -> AnyView {
    let key = move || {
        (
            ui.locale.get(),
            ui.skin.with(|s| Arc::as_ptr(s) as usize),
            ui.settings.with(|s| {
                (
                    s.general.input_mode,
                    s.general.send_key,
                    s.skin.chrome,
                    s.assets.phrases.len(),
                )
            }),
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
            help_open,
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
        let divider = group_divider;
        // Text styling, then colour and size, then the outline and ruby,
        // then what plays (effects, pacing) and what goes in.
        let mut bar: Vec<AnyView> = vec![
            drag_grip(),
            toggle(
                ui,
                ti::BOLD,
                "tb-bold",
                Some("Ctrl+B"),
                || Tool::Bold,
                is_bold,
            ),
            toggle(
                ui,
                ti::ITALIC,
                "tb-italic",
                Some("Ctrl+I"),
                || Tool::Italic,
                is_italic,
            ),
            toggle(
                ui,
                ti::UNDERLINE,
                "tb-underline",
                Some("Ctrl+U"),
                || Tool::Underline,
                is_underline,
            ),
            toggle(
                ui,
                ti::STRIKETHROUGH,
                "tb-strike",
                None,
                || Tool::Strike,
                is_strike,
            ),
            divider(),
            popover(
                ui,
                ti::PALETTE,
                t("tb-color"),
                220.0,
                color_open,
                color_panel(ui, color_open),
            ),
            popover(
                ui,
                ti::TEXT_SIZE,
                t("tb-size"),
                240.0,
                size_open,
                size_panel(ui, size_open),
            ),
        ];
        let stroke = popover(
            ui,
            ti::BORDER_OUTER,
            t("tb-stroke"),
            250.0,
            stroke_open,
            stroke_panel(ui, stroke_open),
        );
        let ruby = popover(
            ui,
            ti::LANGUAGE_HIRAGANA,
            t("tb-ruby"),
            240.0,
            ruby_open,
            ruby_panel(ui, ruby_open),
        );
        let sticker = popover(
            ui,
            ti::STICKER,
            t("tb-sticker"),
            360.0,
            sticker_open,
            sticker_panel(ui, sticker_open),
        );
        let expression = popover(
            ui,
            ti::MOOD_SMILE,
            t("tb-expression"),
            260.0,
            expr_open,
            expression_panel(ui, expr_open),
        );
        let help = popover(
            ui,
            ti::HELP,
            t("tb-cheatsheet"),
            620.0,
            help_open,
            cheat_sheet(ui),
        );
        let effects_menu = menu(ui, ti::SPARKLES, t("tb-effects"), true, fx_open, effects);
        let pacing_menu = menu(ui, ti::HOURGLASS, t("tb-pacing"), true, pace_open, pacing);
        // The popovers of folded tools stay mounted, invisible and out of the
        // flow, so the "…" menu can open them in place.
        let mut hidden: Vec<AnyView> = Vec::new();
        if compact {
            // Clearing formats folds too, so the labelled menus fit beside
            // Send in every language.
            bar.extend([divider(), effects_menu, pacing_menu]);
            let mut items: Vec<AnyView> = vec![
                widget(ActionMenuItem::new(t("tb-clear-format")))
                    .on_cx(move |_, _: &Activate, cx| {
                        more_open.set(false);
                        cx.dispatch_program_all(act(Tool::ClearFormat));
                    })
                    .into_any(),
                open_item(t("tb-stroke"), stroke_open),
                open_item(t("tb-ruby"), ruby_open),
                open_item(t("tb-sticker"), sticker_open),
                open_item(t("tb-expression"), expr_open),
                widget(ActionMenuItem::new(t("tb-source")))
                    .on_cx(move |_, _: &Activate, cx| {
                        more_open.set(false);
                        cx.dispatch_program_all(Msg::ToggleSource);
                    })
                    .into_any(),
                widget(Divider::horizontal()).into_any(),
            ];
            items.extend(inserts);
            bar.push(menu(ui, ti::DOTS, t("tb-more"), false, more_open, items));
            bar.push(help);
            hidden.extend([stroke, ruby, sticker, expression]);
        } else {
            bar.extend([stroke, ruby, divider()]);
            bar.extend([effects_menu, pacing_menu, sticker, expression]);
            bar.push(menu(
                ui,
                ti::TEXT_PLUS,
                t("tb-insert"),
                false,
                insert_open,
                inserts,
            ));
            bar.push(tool(ui, ti::CLEAR_FORMATTING, "tb-clear-format", || {
                act(Tool::ClearFormat)
            }));
            bar.push(source_toggle(ui));
            bar.push(help);
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
            group_divider(),
            mode_switch.into_any(),
            menu(
                ui,
                ti::MESSAGE,
                t("tb-phrases"),
                false,
                phrase_open,
                phrases,
            ),
            tool(ui, ti::SETTINGS, "window-settings", || Msg::OpenSettings),
        ];
        if mode == InputMode::Compose {
            let key = match settings.general.send_key {
                SendKey::Enter => "Enter",
                SendKey::CtrlEnter => "Ctrl+Enter",
            };
            // The same keycaps as the Hotkeys page and the `?` sheet.
            pinned.push(
                widget(Stack::row(0.0).with_layout(|l| {
                    l.flex_shrink = Some(0.0);
                    l.margin_left = Some(LengthSpec::Px(4.0));
                    l.margin_right = Some(LengthSpec::Px(4.0));
                }))
                .children(super::keycap::chord(key))
                .into_any(),
            );
        }
        pinned.push(
            widget(crate::skin_theme::cta(
                Button::new(t("tb-send"))
                    .icon(ti::SEND)
                    .kind(ButtonKind::Primary)
                    .size(ControlSize::Small),
            ))
            .on_cx(move |_, _: &Activate, cx| {
                super::motion::press(cx, ui.reduced_motion.get_untracked());
                cx.dispatch_program_all(Msg::Core(Cmd::Send))
            })
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
                .padding_xy(space::MD, space::SM)
                .align(AlignSpec::Center),
        )
        .children(bar)
        .into_any()
    })
    .into_any()
}

/// A live or attention state as a pill: a highlight wash and outline, the
/// label in body text (pink text would not reach 4.5:1 on the wash), and a
/// leading highlight dot when `dot`.
fn highlight_chip(text: String, dot: bool) -> AnyView {
    let mut st = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.font_size = Some(12.0);
        l.white_space_nowrap = true;
        // "Queued · 2" counts in place.
        super::tnum(l);
    }
    let mut mark = st.clone();
    mark.foreground = Some(SemanticColorRole::Highlight);
    st.foreground = Some(SemanticColorRole::Text);
    let pill = Stack::row(4.0).align(AlignSpec::Center).with_layout(|l| {
        l.flex_shrink = Some(0.0);
        l.padding_left = Some(LengthSpec::Px(8.0));
        l.padding_right = Some(LengthSpec::Px(8.0));
        l.padding_top = Some(LengthSpec::Px(1.0));
        l.padding_bottom = Some(LengthSpec::Px(1.0));
        l.border_radius = Some(999.0);
        l.border_width = Some(1.0);
        l.border_style = Some(nana_ui_core::BorderStyle::Solid);
    });
    let wash = |a: f32| nana_ui_core::SemanticColorMix::alpha(SemanticColorRole::Highlight, a);
    let pill_style = pill
        .node_style()
        .surface_mix(wash(0.16))
        .outline_mix(wash(0.45), 1.0);
    widget(pill.style(pill_style))
        .children((
            dot.then(|| widget(Text::new("●").decorative().style(mark))),
            widget(Text::new(text).decorative().style(st)),
        ))
        .into_any()
}

/// Period of a breathing status pulse (ms).
pub const PULSE_MS: u64 = 1200;

/// Whether the input window needs frames for the mirroring chip's pulse.
pub fn mirror_chip_pulsing(ui: Ui) -> bool {
    !ui.reduced_motion.get_untracked()
        && ui
            .settings
            .with_untracked(|s| s.general.input_mode == InputMode::Mirror)
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
            // Live and attention states take the highlight, not the accent.
            Line::Queued(text) => highlight_chip(text, false),
            Line::Mirror(text) => {
                let mut chip_view =
                    widget(Stack::row(0.0).with_layout(|l| l.flex_shrink = Some(0.0)))
                        .children(highlight_chip(text, true));
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
    })
    // A chip or an error arrives on the `enter` token.
    .transition(super::motion::enter());
    widget(
        Stack::bar(6.0)
            .height(LengthSpec::Px(STATUS_H))
            .padding_xy(space::MD, space::XXS),
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
                l.white_space_nowrap = true;
                l.flex_shrink = Some(0.0);
            }
            // The accent, darkened where it would not read as text.
            label.foreground = Some(SemanticColorRole::AccentOnSoft);
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
    // The placeholder is chrome, not a line: the theme's muted ink with none
    // of the skin's outline or shadow, so it cannot pass for typed text.
    let placeholder_style = move || {
        let mut base = Base::editor(&ui.skin.get());
        base.shadow = None;
        base.stroke = None;
        let mut st = base.node_style();
        let l = Arc::make_mut(&mut st.layout);
        l.position = PositionSpec::Absolute;
        l.offset_left = Some(LengthSpec::Px(0.0));
        l.offset_top = Some(LengthSpec::Px(0.0));
        l.color = None;
        l.pointer_events = Some(PointerEventsSpec::None);
        st.foreground = Some(SemanticColorRole::Muted);
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
                    // Markup source reads in the mono role.
                    super::mono(l);
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

/// While the draft has focus, a 2px ring in the focus accent at 40% just
/// inside the box's region, outside the skin's own border: the input window
/// shows where typing goes, and the display (which has no ring) stays clean.
fn focus_ring(focused: Signal<bool>) -> AnyView {
    widget(
        Stack::column(0.0)
            .outline(SemanticColorRole::FocusBorder, 2.0)
            .radius_px(6.0)
            .with_layout(|l| {
                l.position = PositionSpec::Absolute;
                l.offset_left = Some(LengthSpec::Px(2.0));
                l.offset_top = Some(LengthSpec::Px(2.0));
                l.offset_right = Some(LengthSpec::Px(2.0));
                l.offset_bottom = Some(LengthSpec::Px(2.0));
                l.opacity = Some(0.4);
                l.pointer_events = Some(PointerEventsSpec::None);
            }),
    )
    .visible(move || focused.get())
    .into_any()
}

/// The input window. Returns the editor and the source field for key
/// handling, and the stack the host shows toasts in.
pub fn input_window(ui: Ui) -> WithRefs<impl IntoView, (EditorRef, SourceRef, ToastsRef)> {
    let editor = entity_ref::<RichTextEditor>();
    let source = entity_ref::<TextArea>();
    let toasts = entity_ref::<nana_ui::runtime::ToastStack>();
    let focused = signal(false);
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
    // Summoned, it rises in on the `enter` token.
    .bind(move |s: &mut Stack| {
        let (opacity, transform) = super::motion::rest(ui.input_shown.get());
        restyle(s, |l| {
            l.opacity = Some(opacity);
            l.transform = Some(transform);
        });
    })
    .animate(super::motion::enter_implicit())
    .children((
        toolbar(ui),
        widget(Stack::column(0.0).with_layout(|l| l.position = PositionSpec::Relative))
            .bind(move |s: &mut Stack| {
                let h = region_h();
                let mut taken = std::mem::replace(s, Stack::row(0.0));
                taken = taken.height(LengthSpec::Px(h));
                *s = taken;
            })
            .on(move |e: &FocusWithinChanged| {
                if focused.get_untracked() != e.focused {
                    focused.set(e.focused);
                }
            })
            .children((
                dialogue_box(ui, Source::Draft, Some(editor_area(ui, editor, source))),
                focus_ring(focused),
            )),
        status_line(ui),
        super::feedback::toast_stack(toasts, STATUS_H + 4.0, 2),
    ));
    with_refs(view, (editor, source, toasts))
}

pub type EditorRef = EntityRef<RichTextEditor>;
pub type SourceRef = EntityRef<TextArea>;
pub type ToastsRef = EntityRef<nana_ui::runtime::ToastStack>;
