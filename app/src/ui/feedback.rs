//! Passive feedback shared by the windows: toasts in the corner of the
//! settings, input and chat windows, and the empty states of lists that have
//! nothing in them yet (a ✦ glyph, one line, and a quiet way forward).

use std::sync::Arc;

use nana_ui::icons_tabler as ti;
use nana_ui::runtime::view::{AnyView, EntityRef, IntoView, widget};
use nana_ui::runtime::{Activate, Button, IconGlyph, Stack, Text, ToastStack, ToastTone};
use nana_ui::theme::space;
use nana_ui::{ButtonKind, ControlSize};
use nana_ui_core::{AlignSpec, LengthSpec, PositionSpec, SemanticColorRole};

use crate::msg::Msg;

/// A toast the host is asked to show.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastRequest {
    pub title: String,
    pub tone: ToastTone,
    /// How long it stays, in milliseconds.
    pub ms: u64,
    /// Also show it in the settings window (when open); the input window
    /// shows every toast while it is visible.
    pub settings: bool,
    /// When it was asked for; one no window could show yet waits this long.
    pub at: std::time::Instant,
}

impl ToastRequest {
    pub fn new(title: String, tone: ToastTone, ms: u64, settings: bool) -> Self {
        Self {
            title,
            tone,
            ms,
            settings,
            at: std::time::Instant::now(),
        }
    }
}

/// How long a notice stays as a toast.
pub const NOTICE_MS: u64 = 6000;
/// How long "Sent ✓" stays.
pub const SENT_MS: u64 = 1200;

/// A toast stack in the bottom-right corner of a positioned window root,
/// `bottom` px above its bottom edge. The host pushes toasts into it.
pub fn toast_stack(stack: EntityRef<ToastStack>, bottom: f32, max_visible: usize) -> AnyView {
    let mut st = nana_ui::runtime::NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.position = PositionSpec::Absolute;
        l.offset_right = Some(LengthSpec::Px(space::XL));
        l.offset_bottom = Some(LengthSpec::Px(bottom));
        l.width = Some(LengthSpec::Px(300.0));
        l.max_width = Some(LengthSpec::Percent(90.0));
        l.max_height = Some(LengthSpec::Px(240.0));
        l.align_items = AlignSpec::End;
        l.z_index = Some(100);
    }
    widget(ToastStack::new().max_visible(max_visible).style(st))
        .entity_ref(stack)
        .into_any()
}

/// Lay `view` out with a toast stack over it.
pub fn with_toasts(view: impl IntoView, stack: EntityRef<ToastStack>) -> AnyView {
    with_toasts_limit(view, stack, 3)
}

pub fn with_toasts_limit(
    view: impl IntoView,
    stack: EntityRef<ToastStack>,
    max_visible: usize,
) -> AnyView {
    widget(Stack::fill_column(0.0).with_layout(|l| {
        l.position = PositionSpec::Relative;
        l.width = Some(LengthSpec::Fill);
        l.height = Some(LengthSpec::Fill);
    }))
    .children((view, toast_stack(stack, space::XXXL, max_visible)))
    .into_any()
}

/// An empty list: a ✦ in the accent colour, one muted line and, when
/// there is somewhere to go, a quiet text button.
pub fn empty_state(
    text: String,
    action: Option<(String, Box<dyn Fn() -> Msg + Send + 'static>)>,
) -> AnyView {
    let mut line = nana_ui::runtime::NodeStyle::default();
    Arc::make_mut(&mut line.layout).font_size = Some(13.0);
    line.foreground = Some(SemanticColorRole::Muted);
    let action = action.map(|(label, msg)| {
        widget(
            Button::new(label)
                .kind(ButtonKind::Text)
                .size(ControlSize::Small),
        )
        .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(msg()))
        .into_any()
    });
    widget(
        Stack::row(space::MD)
            .padding_xy(space::XXXL, space::XL)
            .align(AlignSpec::Center)
            .wrap(true),
    )
    .children((
        widget(
            IconGlyph::new(ti::SPARKLES)
                .size(16.0)
                .role(SemanticColorRole::Accent),
        ),
        widget(Text::new(text).style(line)),
        action,
    ))
    .into_any()
}
