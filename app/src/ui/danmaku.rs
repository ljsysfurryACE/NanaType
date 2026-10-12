//! The chat panel: live chat from blivechat for the streamer to read and
//! answer. It is an ordinary window in the app's own look (never captured),
//! so it follows the chrome theme rather than the skin.
//!
//! Each message has two buttons: **@** puts a label with the viewer's avatar
//! and name at the editor's caret, **reply** does the same and quotes the
//! message above the line that is sent next.

use std::sync::Arc;

use nana_ui::ControlSize;
use nana_ui::icons_tabler as ti;
use nana_ui::runtime::view::{AnyView, IntoView, computed, dynamic, each, signal, widget};
use nana_ui::runtime::{
    Activate, IconButton, NodeStyle, RichTextView, ScrollAxes, ScrollView, Stack, Text, UserScroll,
};
use nana_ui_core::{
    AlignSpec, BackgroundImage, BackgroundImageFit, JustifySpec, LengthSpec, OverflowSpec,
    SemanticColorRole,
};
use nanatype_blive::Guard;
use nanatype_settings::{ChatFilter, Settings};

use super::rich::{Env, Look, MentionLook, to_rich};
use super::{I18N, Ui, c};
use crate::chat::{ChatKind, ChatMessage, guard_key, super_chat_color};
use crate::msg::Msg;

const AVATAR: f32 = 30.0;
const TEXT: f32 = 14.0;
/// The row's buttons.
const ACTIONS: f32 = 60.0;
/// List padding, row padding and gaps, and room for the scrollbar.
const CHROME: f32 = 2.0 * 4.0 + 2.0 * 10.0 + 2.0 * 8.0 + 12.0;

/// Connection state, for the dot in the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Link {
    #[default]
    Off,
    Connecting,
    Connected,
    Error,
}

fn text_style(size: f32, weight: u16) -> NodeStyle {
    let mut st = NodeStyle::default();
    let l = Arc::make_mut(&mut st.layout);
    l.font_size = Some(size);
    l.font_weight = Some(weight);
    st
}

fn guard_color(guard: Guard) -> Option<[f32; 4]> {
    match guard {
        Guard::Governor => Some([0.91, 0.30, 0.24, 1.0]),
        Guard::Admiral => Some([0.67, 0.36, 0.89, 1.0]),
        Guard::Captain => Some([0.12, 0.62, 0.86, 1.0]),
        Guard::None => None,
    }
}

/// A round avatar, or the name's first letter on the accent colour.
pub fn avatar(url: Option<String>, name: &str, size: f32, accent: [f32; 4]) -> AnyView {
    let has_url = url.is_some();
    let el = widget(
        Stack::row(0.0)
            .width(LengthSpec::Px(size))
            .height(LengthSpec::Px(size))
            .align(AlignSpec::Center)
            .justify(JustifySpec::Center)
            .with_layout(move |l| {
                l.border_radius = Some(size / 2.0);
                l.flex_shrink = Some(0.0);
                l.overflow_x = OverflowSpec::Hidden;
                l.overflow_y = OverflowSpec::Hidden;
                match url {
                    Some(u) => {
                        l.paint.background_image =
                            Some(BackgroundImage::url_with_fit(u, BackgroundImageFit::Cover))
                    }
                    None => l.background = Some(accent),
                }
            }),
    );
    if has_url {
        return el.into_any();
    }
    let mut st = text_style(size * 0.45, 700);
    Arc::make_mut(&mut st.layout).color = Some([1.0; 4]);
    let initial = name.chars().next().map(String::from).unwrap_or_default();
    el.children(widget(Text::new(initial).decorative().style(st)))
        .into_any()
}

/// A small coloured tag (guard level, medal).
fn badge(text: String, color: [f32; 4]) -> AnyView {
    let mut st = text_style(11.0, 700);
    Arc::make_mut(&mut st.layout).color = Some([1.0; 4]);
    widget(Stack::row(0.0).padding_xy(5.0, 1.0).with_layout(move |l| {
        l.background = Some(color);
        l.border_radius = Some(4.0);
        l.flex_shrink = Some(0.0);
    }))
    .children(widget(Text::new(text).style(st)))
    .into_any()
}

/// The message body as rich text in the panel's look (emoticons inline).
fn body(ui: Ui, m: &ChatMessage, color: Option<[f32; 4]>) -> AnyView {
    let skin = ui.skin.get_untracked();
    let urls = ui.asset_urls.get_untracked();
    let locale = ui.locale.get_untracked();
    let look = Look {
        size: TEXT,
        color: skin.chrome.text,
        weight: 400,
        line_height: 1.4,
        body: None,
        name: None,
        accent: None,
        mention: MentionLook::from_skin(&skin),
    };
    let rich = to_rich(
        &m.display_doc(locale),
        &Env {
            look: &look,
            urls: &urls,
            editor: false,
            locale,
        },
    );
    let mut st = text_style(TEXT, 400);
    {
        let l = Arc::make_mut(&mut st.layout);
        l.line_height = Some(nana_ui_core::LineHeightSpec::Absolute(TEXT * 1.4));
        l.color = color;
    }
    if color.is_none() {
        st.foreground = Some(SemanticColorRole::Text);
    }
    widget(RichTextView::new(rich).style(st).width(LengthSpec::Fill)).into_any()
}

fn action(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    key: &str,
    msg: impl Fn() -> Msg + Send + 'static,
) -> AnyView {
    widget(
        IconButton::new(icon, ui.now_t(key))
            .size(ControlSize::Small)
            .with_tooltip(ui.now_t(key)),
    )
    .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(msg()))
    .into_any()
}

fn row_view(ui: Ui, seq: u64, text_w: f32) -> AnyView {
    let Some(m) = ui
        .chat
        .with_untracked(|rows| rows.iter().rev().find(|r| r.seq == seq).cloned())
    else {
        return widget(Stack::row(0.0)).into_any();
    };
    let skin = ui.skin.get_untracked();
    let accent = c(skin.chrome.accent);
    let locale = ui.locale.get_untracked();
    let url = m.user.avatar.as_ref().and_then(|a| {
        ui.asset_urls
            .with_untracked(|u| u.get(&a.to_string()).cloned())
    });

    let name_color = if m.owner {
        Some([0.96, 0.66, 0.0, 1.0])
    } else if m.admin {
        Some([0.12, 0.53, 0.90, 1.0])
    } else {
        guard_color(m.guard)
    };
    let mut name_st = text_style(13.0, 700);
    match name_color {
        Some(col) => Arc::make_mut(&mut name_st.layout).color = Some(col),
        None => name_st.foreground = Some(SemanticColorRole::Muted),
    }
    let mut badges: Vec<AnyView> = Vec::new();
    if let Some(col) = guard_color(m.guard) {
        badges.push(badge(I18N.t_in(locale, guard_key(m.guard)), col));
    }
    if m.medal_level > 0 && !m.medal_name.is_empty() {
        badges.push(badge(
            format!("{} {}", m.medal_name, m.medal_level),
            [0.45, 0.50, 0.58, 0.9],
        ));
    }
    let time = widget(
        Text::new(super::history::local_time(m.at, true))
            .style(text_style(11.0, 400))
            .color(SemanticColorRole::Faint),
    );
    let header = widget(
        Stack::row(6.0)
            .align(AlignSpec::Center)
            .width(LengthSpec::Fill),
    )
    .children((
        badges,
        widget(Text::new(m.user.name.clone()).style(name_st).truncating()),
        widget(Stack::spacer()),
        time,
    ));

    let (band, content): (Option<[f32; 4]>, AnyView) = match &m.kind {
        ChatKind::Text => (None, body(ui, &m, None)),
        ChatKind::SuperChat { price } => {
            let col = super_chat_color(*price);
            (Some(col), body(ui, &m, Some([0.1, 0.1, 0.12, 1.0])))
        }
        ChatKind::Gift { .. } | ChatKind::Member { .. } => {
            let icon_color = match m.kind {
                ChatKind::Member { .. } => guard_color(m.guard).unwrap_or(accent),
                _ => [0.95, 0.55, 0.20, 1.0],
            };
            let mut st = text_style(13.0, 500);
            Arc::make_mut(&mut st.layout).color = Some(icon_color);
            (
                None,
                widget(
                    Text::new(m.event_text(locale).unwrap_or_default())
                        .style(st)
                        .truncating(),
                )
                .into_any(),
            )
        }
    };

    let seq = m.seq;
    let actions = widget(
        Stack::row(0.0)
            .width(LengthSpec::Px(ACTIONS))
            .justify(JustifySpec::End)
            .with_layout(|l| {
                l.flex_shrink = Some(0.0);
                l.align_self = Some(AlignSpec::Center);
            }),
    )
    .children((
        action(ui, ti::AT, "chat-mention", move || Msg::ChatMention {
            seq,
            reply: false,
        }),
        action(ui, ti::ARROW_BACK_UP, "chat-reply", move || {
            Msg::ChatMention { seq, reply: true }
        }),
    ));
    widget(
        Stack::row(8.0)
            .align(AlignSpec::Start)
            .width(LengthSpec::Fill)
            .padding_xy(10.0, 6.0)
            .with_layout(move |l| {
                if let Some(col) = band {
                    l.background = Some([col[0], col[1], col[2], 0.85]);
                    l.border_radius = Some(8.0);
                }
            }),
    )
    .children((
        avatar(url, &m.user.name, AVATAR, accent),
        // The panel's width less the avatar and the buttons, so the text
        // wraps at the panel's edge.
        widget(
            Stack::column(2.0)
                .width(LengthSpec::Px(text_w))
                .with_layout(|l| {
                    l.flex_shrink = Some(0.0);
                }),
        )
        .children((header, content)),
        actions,
    ))
    .into_any()
}

fn filter_toggle(
    ui: Ui,
    icon: nana_ui::runtime::Icon,
    key: &'static str,
    get: fn(&ChatFilter) -> bool,
    set: fn(&mut ChatFilter, bool),
) -> AnyView {
    widget(
        IconButton::new(icon, ui.now_t(key))
            .size(ControlSize::Small)
            .with_tooltip(ui.now_t(key)),
    )
    .bind(move |b: &mut IconButton| {
        let on = ui.settings.with(|s| get(&s.integrations.blivechat.panel));
        if b.selected != on {
            b.selected = on;
        }
    })
    .on_cx(move |_, _: &Activate, cx| {
        cx.dispatch_program_all(Msg::Edit(Box::new(move |s: &mut Settings| {
            let f = &mut s.integrations.blivechat.panel;
            let on = get(f);
            set(f, !on);
        })))
    })
    .into_any()
}

fn header(ui: Ui) -> AnyView {
    let dot = widget(
        Stack::row(0.0)
            .width(LengthSpec::Px(8.0))
            .height(LengthSpec::Px(8.0)),
    )
    .bind(move |s: &mut Stack| {
        let color = match ui.status.with(|x| x.blive_link) {
            Link::Connected => [0.20, 0.72, 0.42, 1.0],
            Link::Connecting => [0.95, 0.70, 0.20, 1.0],
            Link::Error => [0.90, 0.30, 0.30, 1.0],
            Link::Off => [0.55, 0.55, 0.6, 1.0],
        };
        let mut taken = std::mem::replace(s, Stack::row(0.0));
        taken = taken.with_layout(|l| {
            l.background = Some(color);
            l.border_radius = Some(4.0);
            l.flex_shrink = Some(0.0);
        });
        *s = taken;
    });
    let status = widget(
        Text::new("")
            .style(text_style(12.0, 500))
            .color(SemanticColorRole::Muted)
            .truncating(),
    )
    .bind(move |t: &mut Text| {
        let v = ui.status.with(|s| s.blive.clone());
        if t.value != v {
            t.value = v;
        }
    });
    widget(
        Stack::bar(4.0)
            .height(LengthSpec::Px(42.0))
            .padding_xy(12.0, 4.0)
            .align(AlignSpec::Center),
    )
    .children(vec![
        dot.into_any(),
        widget(Stack::row(0.0).with_layout(|l| {
            l.flex_grow = Some(1.0);
            l.flex_shrink = Some(1.0);
            l.min_width = Some(LengthSpec::Px(0.0));
            l.margin_left = Some(LengthSpec::Px(6.0));
        }))
        .children(status)
        .into_any(),
        filter_toggle(
            ui,
            ti::MESSAGE_CIRCLE,
            "chat-filter-text",
            |f| f.text,
            |f, v| f.text = v,
        ),
        filter_toggle(
            ui,
            ti::COIN,
            "chat-filter-super-chat",
            |f| f.super_chat,
            |f, v| f.super_chat = v,
        ),
        filter_toggle(
            ui,
            ti::GIFT,
            "chat-filter-gift",
            |f| f.gift,
            |f, v| f.gift = v,
        ),
        filter_toggle(
            ui,
            ti::CROWN,
            "chat-filter-member",
            |f| f.member,
            |f, v| f.member = v,
        ),
        action(ui, ti::TRASH, "chat-clear", || Msg::ClearChat),
        action(ui, ti::SETTINGS, "window-settings", || {
            Msg::SettingsPage(super::settings::Page::Integrations)
        }),
    ])
    .into_any()
}

/// Shown when nothing is listed.
fn empty(ui: Ui) -> AnyView {
    widget(
        Stack::column(0.0)
            .padding(24.0)
            .align(AlignSpec::Center)
            .justify(JustifySpec::Center)
            .width(LengthSpec::Fill)
            .with_layout(|l| l.flex_grow = Some(1.0)),
    )
    .children(
        widget(
            Text::new("")
                .style(text_style(13.0, 400))
                .color(SemanticColorRole::Muted),
        )
        .bind(move |t: &mut Text| {
            let configured = ui.settings.with(|s| {
                let b = &s.integrations.blivechat;
                b.enabled && !b.room.is_empty()
            });
            let key = if configured {
                "chat-empty"
            } else {
                "chat-not-configured"
            };
            let v = I18N.t_in(ui.locale.get(), key);
            if t.value != v {
                t.value = v;
            }
        }),
    )
    .into_any()
}

/// The keys of the messages the panel lists, oldest first.
pub fn shown(rows: &[Arc<ChatMessage>], filter: &ChatFilter) -> Vec<u64> {
    rows.iter()
        .filter(|m| m.passes(filter))
        .map(|m| m.seq)
        .collect()
}

pub fn chat_window(ui: Ui) -> impl IntoView {
    let items = computed(move || {
        let filter = ui.settings.with(|s| s.integrations.blivechat.panel.clone());
        ui.chat.with(|rows| shown(rows, &filter))
    });
    // Following the newest message until the user scrolls up to read.
    let follow = signal(true);
    let list = widget(ScrollView::new(ScrollAxes::Vertical).follow_end(true))
        .bind(move |v: &mut ScrollView| {
            let on = follow.get();
            if v.follow_end != on {
                v.follow_end = on;
            }
        })
        .on(move |e: &UserScroll| {
            if follow.get_untracked() != e.at_end {
                follow.set(e.at_end);
            }
        })
        .children(dynamic(
            move || ui.chat_width.get().round() as i32,
            move |width| {
                let text_w = (*width as f32 - AVATAR - ACTIONS - CHROME).max(120.0);
                widget(
                    Stack::column(2.0)
                        .padding_xy(4.0, 6.0)
                        .width(LengthSpec::Fill),
                )
                .children(each(
                    items,
                    |k: &u64| *k,
                    move |k: u64| row_view(ui, k, text_w),
                ))
                .into_any()
            },
        ));
    widget(
        Stack::fill_column(0.0)
            .surface(SemanticColorRole::Background)
            .with_layout(|l| {
                l.overflow_x = OverflowSpec::Hidden;
                l.overflow_y = OverflowSpec::Hidden;
            }),
    )
    .children((
        header(ui),
        widget(Stack::fill_column(0.0).with_layout(|l| {
            l.flex_grow = Some(1.0);
            l.min_height = Some(LengthSpec::Px(0.0));
        }))
        .children((
            widget(Stack::fill_column(0.0))
                .visible(move || !items.with(|i| i.is_empty()))
                .children(list),
            widget(Stack::fill_column(0.0))
                .visible(move || items.with(|i| i.is_empty()))
                .children(empty(ui)),
        )),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanatype_model::{ChatUser, Document};

    fn message(seq: u64, kind: ChatKind) -> Arc<ChatMessage> {
        Arc::new(ChatMessage {
            seq,
            id: seq.to_string(),
            kind,
            user: ChatUser::default(),
            guard: Guard::None,
            owner: false,
            admin: false,
            medal_level: 0,
            medal_name: String::new(),
            doc: Document::plain("hi"),
            at: 1,
        })
    }

    #[test]
    fn the_panel_lists_what_its_filter_lets_through() {
        let rows = vec![
            message(1, ChatKind::Text),
            message(2, ChatKind::SuperChat { price: 30 }),
            message(
                3,
                ChatKind::Gift {
                    name: "x".into(),
                    num: 1,
                    coin: 0,
                },
            ),
        ];
        assert_eq!(shown(&rows, &ChatFilter::default()), [1, 2, 3]);
        let filter = ChatFilter {
            text: false,
            min_gift_coin: 1,
            ..ChatFilter::default()
        };
        assert_eq!(shown(&rows, &filter), [2]);
    }
}
