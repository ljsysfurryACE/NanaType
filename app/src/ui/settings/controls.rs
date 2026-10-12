//! Settings building blocks: a section, a labelled row, and controls bound to
//! one settings field each.

use std::sync::{Arc, LazyLock};

use nana_ui::runtime::view::{AnyView, IntoView, settings_row, widget};
use nana_ui::runtime::{
    ColorChanged, ColorField, NodeStyle, NumberChanged, NumberInput, RangeChanged, RangeField,
    Select, SelectChanged, SelectOption, Stack, StatusBadge, Switch, Text, TextChanged, TextInput,
    ToggleChanged,
};
use nana_ui::{ControlSize, StatusTone};
use nana_ui_core::{AlignSpec, LengthSpec, SemanticColorRole};
use nanatype_model::Rgba8;
use nanatype_settings::Settings;

use crate::msg::Msg;
use crate::ui::input::PULSE_MS;
use crate::ui::{I18N, LinkState, Status, Ui};

pub static ICON_PNG_B64: LazyLock<String> =
    LazyLock::new(|| base64(include_bytes!("../../../assets/tray-64.png")));

pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn text_style(size: f32, weight: u16) -> NodeStyle {
    let mut st = NodeStyle::default();
    let l = Arc::make_mut(&mut st.layout);
    l.font_size = Some(size);
    l.font_weight = Some(weight);
    st
}

pub fn heading(ui: Ui, key: &str) -> AnyView {
    widget(Text::new(ui.now_t(key)).style(text_style(22.0, 700))).into_any()
}

pub fn label(text: String) -> AnyView {
    widget(Text::new(text).style(text_style(14.0, 500))).into_any()
}

pub fn hint(text: String) -> AnyView {
    widget(
        Text::new(text)
            .style(text_style(12.0, 400))
            .color(SemanticColorRole::Muted),
    )
    .into_any()
}

/// Rows with a hairline between each.
fn separated(rows: Vec<AnyView>) -> Vec<AnyView> {
    let n = rows.len();
    rows.into_iter()
        .enumerate()
        .map(|(i, r)| {
            widget(Stack::column(0.0).with_layout(move |l| {
                if i + 1 < n {
                    l.border_bottom_width = Some(1.0);
                    l.border_bottom_color = Some([0.5, 0.5, 0.5, 0.12]);
                    l.border_bottom_style = Some(nana_ui_core::BorderStyle::Solid);
                }
            }))
            .children(r)
            .into_any()
        })
        .collect()
}

/// A titled group of rows.
pub fn section(ui: Ui, title_key: &str, rows: Vec<AnyView>) -> AnyView {
    let rows = separated(rows);
    widget(Stack::column(0.0).with_layout(|l| {
        l.margin_top = Some(LengthSpec::Px(18.0));
    }))
    .children((
        widget(
            Text::new(ui.now_t(title_key))
                .style(text_style(13.0, 700))
                .color(SemanticColorRole::Muted),
        )
        .into_any(),
        widget(
            Stack::column(0.0)
                .surface(SemanticColorRole::Surface)
                .radius(nana_ui_core::RadiusTier::Lg)
                .outline(SemanticColorRole::BorderSoft, 1.0)
                .padding_xy(4.0, 2.0)
                .with_layout(|l| {
                    l.margin_top = Some(LengthSpec::Px(8.0));
                }),
        )
        .children(rows),
    ))
    .into_any()
}

/// The rows that only mean something while a master toggle is on. They fold
/// away with it, leaving a one-line hint, so a switched-off feature does not
/// show a page of dead settings.
pub fn when_on(
    ui: Ui,
    enabled: fn(&Settings) -> bool,
    rows: Vec<AnyView>,
    off_hint_key: &'static str,
) -> AnyView {
    widget(Stack::column(0.0))
        .children((
            widget(Stack::column(0.0))
                .visible(move || ui.settings.with(|s| enabled(s)))
                .children(separated(rows)),
            widget(Stack::column(0.0).padding_xy(16.0, 12.0))
                .visible(move || !ui.settings.with(|s| enabled(s)))
                .children(hint(ui.now_t(off_hint_key))),
        ))
        .into_any()
}

/// Whether any link badge on the visible page is breathing, so the settings
/// window needs frames for it.
pub fn links_pulsing(ui: Ui) -> bool {
    !ui.reduced_motion.get_untracked()
        && matches!(
            ui.page.get_untracked(),
            super::Page::Integrations | super::Page::Output
        )
        && ui.status.with_untracked(|s| {
            [&s.vts, &s.osc, &s.ws, &s.spout_display, &s.spout_history]
                .into_iter()
                .any(|l| *l == LinkState::Connecting)
                || s.blive_link == crate::ui::danmaku::Link::Connecting
        })
}

/// A link's state as a dot-and-text badge: green connected, amber connecting
/// or waiting for access, red failed, grey off. Connecting breathes unless
/// motion is reduced.
pub fn link_badge(ui: Ui, get: fn(&Status) -> &LinkState) -> AnyView {
    let describe = move || {
        let locale = ui.locale.get();
        ui.status.with(|s| match get(s) {
            LinkState::Off => (I18N.t_in(locale, "int-status-off"), StatusTone::Neutral),
            LinkState::Connecting => (
                I18N.t_in(locale, "int-status-connecting"),
                StatusTone::Warning,
            ),
            LinkState::Connected(text) => (text.clone(), StatusTone::Success),
            LinkState::NeedsAuth => (
                I18N.t_in(locale, "int-status-needs-token"),
                StatusTone::Warning,
            ),
            LinkState::Error(text) => (text.clone(), StatusTone::Danger),
        })
    };
    breathing_badge(ui, describe, move || {
        ui.status.with(|st| *get(st) == LinkState::Connecting)
    })
}

/// blivechat's badge: its own status line and the chat panel's dot state,
/// drawn like the other links.
pub fn blive_badge(ui: Ui) -> AnyView {
    use crate::ui::danmaku::Link;
    let describe = move || {
        let locale = ui.locale.get();
        ui.status.with(|s| {
            let text = if s.blive.is_empty() {
                I18N.t_in(locale, "int-status-off")
            } else {
                s.blive.clone()
            };
            let tone = match s.blive_link {
                Link::Off => StatusTone::Neutral,
                Link::Connecting => StatusTone::Warning,
                Link::Connected => StatusTone::Success,
                Link::Error => StatusTone::Danger,
            };
            (text, tone)
        })
    };
    breathing_badge(ui, describe, move || {
        ui.status.with(|st| st.blive_link == Link::Connecting)
    })
}

fn breathing_badge(
    ui: Ui,
    describe: impl Fn() -> (String, StatusTone) + Send + Sync + Copy + 'static,
    connecting: impl Fn() -> bool + Send + Sync + Copy + 'static,
) -> AnyView {
    let (text, tone) = describe();
    let badge =
        widget(StatusBadge::new(text, tone).compact(true)).bind(move |b: &mut StatusBadge| {
            let (text, tone) = describe();
            let text: Arc<str> = text.into();
            if b.label != text {
                b.label = text;
            }
            b.tone = tone;
        });
    widget(Stack::row(0.0).with_layout(|l| l.flex_shrink = Some(0.0)))
        .bind(move |s: &mut Stack| {
            let connecting = connecting();
            let opacity = if connecting && !ui.reduced_motion.get() {
                let phase = (ui.pulse_ms.get() % PULSE_MS) as f32 / PULSE_MS as f32;
                let wave = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
                0.55 + 0.45 * wave
            } else {
                1.0
            };
            crate::ui::restyle(s, |l| l.opacity = Some(opacity));
        })
        .children(badge)
        .into_any()
}

/// Label (and hint) on the left, control on the right; stacks when narrow.
pub fn row(ui: Ui, label_key: &str, hint_key: Option<&str>, control: AnyView) -> AnyView {
    row_text(ui.now_t(label_key), hint_key.map(|k| ui.now_t(k)), control)
}

pub fn row_text(label_text: String, hint_text: Option<String>, control: AnyView) -> AnyView {
    let hint: Option<Arc<str>> = hint_text.map(Into::into);
    settings_row(label_text)
        .hint(hint)
        .control(widget(Stack::row(8.0).align(AlignSpec::Center)).children(control))
        .into_any()
}

pub fn edit(f: impl FnOnce(&mut Settings) + Send + 'static) -> Msg {
    Msg::Edit(Box::new(f))
}

pub fn switch(ui: Ui, get: fn(&Settings) -> bool, set: fn(&mut Settings, bool)) -> AnyView {
    widget(Switch::new("", get(&ui.settings.get_untracked())))
        .bind(move |s: &mut Switch| s.checked = ui.settings.with(|x| get(x)))
        .on_cx(move |_, e: &ToggleChanged, cx| {
            let v = e.checked;
            cx.dispatch_program_all(edit(move |s| set(s, v)));
        })
        .into_any()
}

/// A select whose options are `(value, label)`.
pub fn select(
    ui: Ui,
    options: Vec<(String, String)>,
    get: impl Fn(&Settings) -> String + Send + Sync + Copy + 'static,
    set: impl Fn(&mut Settings, &str) + Send + Sync + Copy + 'static,
) -> AnyView {
    let opts: Vec<SelectOption> = options
        .iter()
        .map(|(v, l)| SelectOption::new(v.clone(), l.clone()))
        .collect();
    widget(Stack::row(0.0).width(LengthSpec::Px(240.0)))
        .children(
            widget(
                Select::new(Some(get(&ui.settings.get_untracked())))
                    .options(opts)
                    .size(ControlSize::Small),
            )
            .bind(move |s: &mut Select| {
                let v: Arc<str> = ui.settings.with(|x| get(x)).into();
                if s.value.as_ref() != Some(&v) {
                    s.value = Some(v);
                }
            })
            .on_cx(move |_, e: &SelectChanged, cx| {
                let v = e.value.to_string();
                cx.dispatch_program_all(edit(move |s| set(s, &v)));
            }),
        )
        .into_any()
}

pub fn select_keys(ui: Ui, options: &[(&'static str, &'static str)]) -> Vec<(String, String)> {
    let locale = ui.locale.get_untracked();
    options
        .iter()
        .map(|(v, k)| ((*v).to_owned(), I18N.t_in(locale, k)))
        .collect()
}

pub fn slider(
    ui: Ui,
    range: (f64, f64, f64),
    unit: &str,
    get: fn(&Settings) -> f64,
    set: fn(&mut Settings, f64),
) -> AnyView {
    let (min, max, step) = range;
    widget(Stack::row(0.0).width(LengthSpec::Px(240.0)))
        .children(
            widget(
                RangeField::new(
                    get(&ui.settings.get_untracked()).clamp(min, max),
                    min,
                    max,
                    step,
                )
                .unit(unit.to_owned())
                .show_label(false),
            )
            .bind(move |r: &mut RangeField| {
                let v = ui.settings.with(|x| get(x)).clamp(min, max);
                if (r.value - v).abs() > f64::EPSILON {
                    r.value = v;
                }
            })
            .on_cx(move |_, e: &RangeChanged, cx| {
                let v = e.value;
                cx.dispatch_program_all(edit(move |s| set(s, v)));
            }),
        )
        .into_any()
}

pub fn number(
    ui: Ui,
    range: (f64, f64),
    get: fn(&Settings) -> f64,
    set: fn(&mut Settings, f64),
) -> AnyView {
    widget(Stack::row(0.0).width(LengthSpec::Px(140.0)))
        .children(
            widget(
                NumberInput::new(get(&ui.settings.get_untracked()))
                    .range(range.0, range.1)
                    .size(ControlSize::Small),
            )
            .on_cx(move |_, e: &NumberChanged, cx| {
                let v = e.value;
                cx.dispatch_program_all(edit(move |s| set(s, v)));
            }),
        )
        .into_any()
}

pub fn text(
    ui: Ui,
    placeholder: String,
    width: f32,
    get: fn(&Settings) -> String,
    set: fn(&mut Settings, String),
) -> AnyView {
    widget(Stack::row(0.0).width(LengthSpec::Px(width)))
        .children(
            widget(
                TextInput::new(get(&ui.settings.get_untracked()))
                    .placeholder(placeholder)
                    .size(ControlSize::Small),
            )
            .on_cx(move |_, e: &TextChanged, cx| {
                let v = e.value.to_string();
                cx.dispatch_program_all(edit(move |s| set(s, v)));
            }),
        )
        .into_any()
}

pub fn color(
    ui: Ui,
    fallback: Rgba8,
    get: fn(&Settings) -> Option<Rgba8>,
    set: fn(&mut Settings, Option<Rgba8>),
) -> AnyView {
    let start = get(&ui.settings.get_untracked()).unwrap_or(fallback);
    widget(Stack::row(0.0).width(LengthSpec::Px(200.0)))
        .children(
            widget(ColorField::new(crate::ui::c(start)).size(ControlSize::Small)).on_cx(
                move |_, e: &ColorChanged, cx| {
                    let [r, g, b, a] = e.value;
                    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                    let c = Rgba8::rgba(to(r), to(g), to(b), to(a));
                    cx.dispatch_program_all(edit(move |s| set(s, Some(c))));
                },
            ),
        )
        .into_any()
}
