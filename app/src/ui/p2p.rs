use nana_ui::runtime::view::{AnyView, IntoView, dynamic, signal, widget};
use nana_ui::runtime::{Activate, Button, Stack, Text, TextChanged, TextInput};
use nana_ui::{ButtonKind, ControlSize};
use nana_ui_core::{AlignSpec, LengthSpec};
use nanatype_p2p::Status;

use super::Ui;
use crate::msg::Msg;

pub fn card_height(ui: Ui) -> f32 {
    if ui.peer_panel.get_untracked() {
        150.0
    } else {
        36.0
    }
}

fn field() -> Stack {
    Stack::column(0.0)
        .width(LengthSpec::Fill)
        .with_layout(|layout| {
            layout.flex_grow = Some(1.0);
            layout.flex_shrink = Some(1.0);
            layout.min_width = Some(LengthSpec::Px(0.0));
            layout.width = Some(LengthSpec::Px(0.0));
        })
}

fn action() -> Stack {
    Stack::row(0.0)
        .width(LengthSpec::Px(100.0))
        .with_layout(|layout| {
            layout.flex_shrink = Some(0.0);
        })
}

pub fn card(ui: Ui) -> AnyView {
    let invitation = signal(String::new());
    let relay = signal(String::new());
    dynamic(
        move || (ui.peer_panel.get(), ui.locale.get(), ui.peer.get()),
        move |(expanded, _, state)| {
            let expanded = *expanded;
            let status = match &state.status {
                Status::Starting => ui.now_t("peer-connecting"),
                Status::Waiting => ui.now_t("peer-waiting"),
                Status::Reconnecting => ui.now_t("peer-reconnecting"),
                Status::Connected { route, rtt_ms } => format!(
                    "{} · {} · {} ms",
                    state.name,
                    ui.now_t(match route.as_str() {
                        "relay" => "peer-relay",
                        "direct" => "peer-direct",
                        _ => "peer-connecting",
                    }),
                    rtt_ms
                ),
                Status::Ended => ui.now_t("peer-off"),
                Status::Error(error) => format!("{}: {error}", ui.now_t("peer-error")),
            };
            let header = widget(
                Stack::row(8.0)
                    .width(LengthSpec::Fill)
                    .align(AlignSpec::Center)
                    .height(LengthSpec::Px(32.0)),
            )
            .children((
                widget(field()).children(widget(
                    Text::new(format!("{} · {status}", ui.now_t("peer-title"))).style({
                        let mut style = nana_ui::runtime::NodeStyle::default();
                        let layout = std::sync::Arc::make_mut(&mut style.layout);
                        layout.white_space_nowrap = true;
                        layout.text_overflow_ellipsis = true;
                        layout.overflow_x = nana_ui_core::OverflowSpec::Hidden;
                        style
                    }),
                )),
                widget(
                    Button::new(ui.now_t(if expanded {
                        "peer-collapse"
                    } else {
                        "peer-expand"
                    }))
                    .size(ControlSize::Small),
                )
                .on_cx(move |_, _: &Activate, cx| {
                    cx.dispatch_program_all(Msg::PeerPanel(!expanded))
                }),
            ))
            .into_any();
            if !expanded {
                return widget(Stack::column(0.0).padding(2.0).height(LengthSpec::Px(36.0)))
                    .children(header)
                    .into_any();
            }
            let body = if state.active {
                let code = state.invitation.clone();
                widget(Stack::column(8.0))
                    .children((
                        widget(Text::new(ui.now_t("peer-own-controls"))),
                        widget(Stack::row(8.0)).children((
                            (!code.is_empty()).then(|| {
                                widget(Button::new(ui.now_t("peer-copy")).size(ControlSize::Small))
                                    .on_cx(move |_, _: &Activate, cx| {
                                        cx.dispatch_program_all(Msg::CopyText(code.clone()))
                                    })
                            }),
                            widget(
                                Button::new(ui.now_t("peer-window-toggle"))
                                    .size(ControlSize::Small),
                            )
                            .on_cx(move |_, _: &Activate, cx| {
                                cx.dispatch_program_all(Msg::ToggleWindow(
                                    crate::windows::WindowKind::PeerDisplay,
                                ))
                            }),
                            widget(Button::new(ui.now_t("peer-leave")).size(ControlSize::Small))
                                .on_cx(move |_, _: &Activate, cx| {
                                    cx.dispatch_program_all(Msg::PeerLeave)
                                }),
                        )),
                    ))
                    .into_any()
            } else {
                widget(Stack::column(6.0))
                    .children((
                        widget(Stack::row(6.0).width(LengthSpec::Fill)).children((
                            widget(field()).children(
                                widget(
                                    TextInput::new(invitation.get_untracked())
                                        .placeholder(ui.now_t("peer-invitation"))
                                        .size(ControlSize::Small),
                                )
                                .on(move |e: &TextChanged| invitation.set(e.value.to_string())),
                            ),
                            widget(action()).children(
                                widget(Button::new(ui.now_t("peer-join")).size(ControlSize::Small))
                                    .on_cx(move |_, _: &Activate, cx| {
                                        cx.dispatch_program_all(Msg::PeerStart {
                                            invitation: Some(invitation.get_untracked()),
                                            relay: relay.get_untracked(),
                                        })
                                    }),
                            ),
                        )),
                        widget(Stack::row(6.0).width(LengthSpec::Fill)).children((
                            widget(field()).children(
                                widget(
                                    TextInput::new(relay.get_untracked())
                                        .placeholder(ui.now_t("peer-relay-placeholder"))
                                        .size(ControlSize::Small),
                                )
                                .on(move |e: &TextChanged| relay.set(e.value.to_string())),
                            ),
                            widget(action()).children(
                                widget(
                                    Button::new(ui.now_t("peer-create"))
                                        .kind(ButtonKind::Primary)
                                        .size(ControlSize::Small),
                                )
                                .on_cx(
                                    move |_, _: &Activate, cx| {
                                        cx.dispatch_program_all(Msg::PeerStart {
                                            invitation: None,
                                            relay: relay.get_untracked(),
                                        })
                                    },
                                ),
                            ),
                        )),
                    ))
                    .into_any()
            };
            widget(
                Stack::column(6.0)
                    .padding(6.0)
                    .height(LengthSpec::Px(150.0)),
            )
            .children((header, body))
            .into_any()
        },
    )
    .into_any()
}
