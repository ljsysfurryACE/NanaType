//! Keycaps: keys drawn as small raised caps, wherever the app names a key
//! (the hotkey rows in settings, the input window's shortcut sheet).

use std::sync::Arc;

use nana_ui::runtime::view::{AnyView, IntoView, widget};
use nana_ui::runtime::{IconGlyph, NodeStyle, Stack, Text};
use nana_ui::theme::space;
use nana_ui_core::{AlignSpec, BoxShadowSpec, JustifySpec, LengthSpec, SemanticColorRole};

fn label_style(weight: u16) -> NodeStyle {
    let mut st = NodeStyle::default();
    let l = Arc::make_mut(&mut st.layout);
    l.font_size = Some(12.0);
    l.font_weight = Some(weight);
    l.white_space_nowrap = true;
    super::tnum(l);
    st
}

/// The face of a keycap: a raised 24px cap on the surface with a soft
/// border and an inner bottom edge. Callers put the key's label inside
/// ([`keycap`]) or bind one themselves.
pub fn face() -> Stack {
    Stack::row(0.0)
        .height(LengthSpec::Px(24.0))
        .min_width(LengthSpec::Px(24.0))
        .padding_xy(space::LG, 0.0)
        .align(AlignSpec::Center)
        .justify(JustifySpec::Center)
        .surface(SemanticColorRole::Surface)
        .outline(SemanticColorRole::BorderSoft, 1.0)
        .radius(nana_ui_core::RadiusTier::Sm)
        .with_layout(|l| {
            l.flex_shrink = Some(0.0);
            l.paint.box_shadows = vec![BoxShadowSpec {
                paint_color: None,
                offset_x: 0.0,
                offset_y: -2.0,
                blur_radius: 0.0,
                spread_radius: 0.0,
                color: [0.5, 0.55, 0.62, 0.28],
                inset: true,
            }];
        })
}

/// The label inside a keycap face.
pub fn label(key: impl Into<String>) -> Text {
    Text::new(key.into())
        .style(label_style(600))
        .color(SemanticColorRole::Text)
}

/// One key as a keycap. Arrow keys show a drawn arrow: the text arrows
/// (↑ ↓ ← →) come from a fallback font in most UI faces and sit oversized
/// and off the baseline next to the letters.
pub fn keycap(key: impl Into<String>) -> AnyView {
    let key = key.into();
    match arrow(&key) {
        Some(icon) => {
            let mut glyph = IconGlyph::new(icon).size(12.0).decorative();
            glyph.style.foreground = Some(SemanticColorRole::Text);
            widget(face()).children(widget(glyph)).into_any()
        }
        None => widget(face())
            .children(widget(label(key).decorative()))
            .into_any(),
    }
}

/// The drawn arrow for an arrow key's label.
fn arrow(key: &str) -> Option<nana_ui::runtime::Icon> {
    use nana_ui::icons_tabler as ti;
    Some(match key {
        "↑" | "Up" | "ArrowUp" => ti::ARROW_UP,
        "↓" | "Down" | "ArrowDown" => ti::ARROW_DOWN,
        "←" | "Left" | "ArrowLeft" => ti::ARROW_LEFT,
        "→" | "Right" | "ArrowRight" => ti::ARROW_RIGHT,
        _ => return None,
    })
}

/// The muted "+" between the keys of a chord.
pub fn plus() -> Text {
    Text::new("+")
        .style(label_style(500))
        .color(SemanticColorRole::Muted)
}

/// A chord ("Ctrl+B") as keycaps joined by "+".
pub fn chord(keys: &str) -> AnyView {
    let mut caps: Vec<AnyView> = Vec::new();
    for (i, key) in keys.split('+').enumerate() {
        if i > 0 {
            caps.push(widget(plus().decorative()).into_any());
        }
        caps.push(keycap(key));
    }
    widget(Stack::row(4.0).align(AlignSpec::Center))
        .children(caps)
        .into_any()
}
