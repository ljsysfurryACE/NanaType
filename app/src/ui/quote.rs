//! The chat message a line answers, quoted in one small line above it:
//! avatar · name: text. The dialogue box and history both draw it from the
//! skin's `reply` spec.

use std::sync::Arc;

use nana_ui::runtime::view::{AnyView, IntoView, widget};
use nana_ui::runtime::{NodeStyle, Stack, Text};
use nana_ui_core::{AlignSpec, LengthSpec};
use nanatype_model::{ChatRef, Rgba8};
use nanatype_theme::{ReplySpec, Skin};

use super::{Ui, c};

/// `reply` as a quote line, sized from `text_size` and coloured from
/// `text_color` unless the spec says otherwise.
pub fn quote_view(
    ui: Ui,
    skin: &Skin,
    spec: &ReplySpec,
    reply: &ChatRef,
    text_size: f32,
    text_color: Rgba8,
) -> AnyView {
    let size = spec
        .size
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or((text_size * 0.62).max(10.0));
    let color = spec.color.map_or(text_color, |col| skin.color(col));
    let name_color = skin.chrome.accent;
    let style = |color: Rgba8, weight: u16| {
        let mut st = NodeStyle::default();
        let l = Arc::make_mut(&mut st.layout);
        l.font_size = Some(size);
        l.font_weight = Some(weight);
        l.color = Some(c(color));
        l.white_space_nowrap = true;
        st
    };
    let url = reply.user.avatar.as_ref().and_then(|a| {
        ui.asset_urls
            .with_untracked(|u| u.get(&a.to_string()).cloned())
    });
    let avatar = super::danmaku::avatar(
        url,
        &reply.user.name,
        (size * 1.4).round(),
        c(skin.chrome.accent),
    );
    let opacity = spec.opacity.clamp(0.0, 1.0);
    widget(
        Stack::row((size * 0.4).round())
            .align(AlignSpec::Center)
            .width(LengthSpec::Fill)
            .with_layout(move |l| {
                l.opacity = Some(opacity);
                l.margin_bottom = Some(LengthSpec::Px(4.0));
                l.flex_shrink = Some(0.0);
                l.min_width = Some(LengthSpec::Px(0.0));
            }),
    )
    .children((
        avatar,
        widget(Text::new(format!("{}：", reply.user.name)).style({
            let mut st = style(name_color, 700);
            Arc::make_mut(&mut st.layout).flex_shrink = Some(0.0);
            st
        })),
        widget(Stack::row(0.0).width(LengthSpec::Fill).with_layout(|l| {
            l.flex_shrink = Some(1.0);
            l.min_width = Some(LengthSpec::Px(0.0));
        }))
        .children(widget(
            Text::new(reply.text.clone())
                .style(style(color, 400))
                .truncating(),
        )),
    ))
    .into_any()
}
