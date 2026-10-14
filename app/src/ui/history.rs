//! The history window: a blivechat-style column of avatar, name and bubble.
//! Rows are drawn by the same document builder as the dialogue box, so a line
//! keeps its colours, effects and stickers; with "typing along" sync the
//! newest row reveals in step with the typewriter.

use std::sync::Arc;

use nana_ui::runtime::view::{AnyView, IntoView, Transition, computed, dynamic, each, widget};
use nana_ui::runtime::{NodeStyle, ScrollAxes, ScrollView, Stack, Text};
use nana_ui_core::{
    AlignSpec, BackgroundImage, CssGradient, Easing, GradientStop, LengthSpec, LinearGradient,
    OverflowSpec, PointerEventsSpec, PositionSpec,
};
use nanatype_core::HistoryRow;
use nanatype_model::{Entrance, HistoryItem, LineId, Rgba8, SourceKind};
use nanatype_theme::{
    AvatarShape, DecorLayer, HistoryAnchor, HistoryEntrance, HistoryLayout, HistoryTitle, Skin,
};

use super::dialogue::{self, Base, Timing, document};
use super::surface::{self, Surface};
use super::{Ui, c};

pub fn local_hm(unix_ms: u64) -> String {
    local_time(unix_ms, false)
}

pub fn local_time(unix_ms: u64, seconds: bool) -> String {
    let offset = crate::localtime::offset_seconds();
    let secs = (unix_ms / 1000) as i64 + offset;
    let day = secs.rem_euclid(86_400);
    let hm = format!("{:02}:{:02}", day / 3600, (day % 3600) / 60);
    if seconds {
        format!("{hm}:{:02}", day % 60)
    } else {
        hm
    }
}

/// How long a new row takes to settle in.
const ROW_ENTER_MS: u64 = 220;

/// Meta text (time, role) is a step below the name: 11px beside a 14px name.
fn time_size(h: &nanatype_theme::History) -> f32 {
    h.time_size.unwrap_or((h.name_size - 3.0).max(9.0))
}

/// `col` mixed `t` of the way to white.
fn lighten(col: Rgba8, t: f32) -> Rgba8 {
    let m = |x: u8| (f32::from(x) + (255.0 - f32::from(x)) * t).round() as u8;
    Rgba8::rgba(m(col.r), m(col.g), m(col.b), col.a)
}

/// The streamer's picture-less avatar: the accent, lighter towards the
/// bottom right.
fn avatar_gradient(from: Rgba8, to: Rgba8) -> BackgroundImage {
    let stop = |position: f32, col: Rgba8| GradientStop {
        paint_color: None,
        position,
        color: c(col),
    };
    BackgroundImage::Gradient(CssGradient::Linear(LinearGradient {
        angle_deg: 135.0,
        stops: vec![stop(0.0, from), stop(1.0, to)],
    }))
}

/// Space the list loses to the window padding, or to the panel's margin,
/// border and padding.
fn list_inset(h: &nanatype_theme::History) -> f32 {
    match &h.panel {
        Some(p) => {
            let [_, right, _, left] = p.shape.border_widths(p.border_width);
            p.margin[1] + p.margin[3] + left + right + p.padding[0] + p.padding[2]
        }
        None => 16.0 + 8.0,
    }
}

fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "·".into())
}

/// Width available to a row's text column.
fn column_width(skin: &nanatype_theme::Skin, settings: &nanatype_settings::Settings) -> f32 {
    let (w, _) = crate::windows::size_of(crate::windows::WindowKind::History, skin, settings);
    let avatar = if settings.history.show_avatar && skin.history.avatar.shape != AvatarShape::None {
        skin.history.avatar.size + 10.0
    } else {
        0.0
    };
    (w - list_inset(&skin.history) - avatar).max(80.0)
}

/// Viewers' avatars and name pills take one of these, by who they are.
const VIEWER_COLORS: [Rgba8; 6] = [
    Rgba8::rgb(0xff, 0x9a, 0xb5),
    Rgba8::rgb(0x7e, 0xc8, 0xf5),
    Rgba8::rgb(0xff, 0xc9, 0x4d),
    Rgba8::rgb(0x8f, 0xd1, 0x8a),
    Rgba8::rgb(0xb7, 0x9c, 0xf2),
    Rgba8::rgb(0xff, 0xa7, 0x7a),
];
/// Text on a viewer colour (all of them are light).
const VIEWER_INK: Rgba8 = Rgba8::rgb(16, 22, 32);

/// A viewer's colour: stable for a user id, else for the name.
fn viewer_color(item: &HistoryItem) -> Rgba8 {
    let id = item.uid.as_deref().unwrap_or(&item.author);
    // FNV-1a, so the colour does not change between runs.
    let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    VIEWER_COLORS[(hash % VIEWER_COLORS.len() as u64) as usize]
}

/// Rows of one speaker this close together share a header.
const GROUP_MS: u64 = 120_000;
/// Space between the rows of a group.
const GROUP_GAP: f32 = 4.0;

/// Whether `row` continues `prev`'s group: same speaker, soon after.
fn continues(prev: &HistoryItem, row: &HistoryItem) -> bool {
    !row.author.is_empty()
        && prev.kind == row.kind
        && prev.author == row.author
        && prev.uid == row.uid
        && row.at >= prev.at
        && row.at - prev.at < GROUP_MS
}

/// The shown rows that continue the row above them.
fn follow_ups(rows: &[HistoryRow], shown: &[u64]) -> Vec<u64> {
    let item = |k: u64| rows.iter().find(|r| r.key == k).map(|r| &r.item);
    shown
        .windows(2)
        .filter(|w| matches!((item(w[0]), item(w[1])), (Some(a), Some(b)) if continues(a, b)))
        .map(|w| w[1])
        .collect()
}

fn row_view(ui: Ui, key: u64, follows: bool) -> AnyView {
    let skin = ui.skin.get_untracked();
    let settings = ui.settings.get_untracked();
    let h = skin.history.clone();
    let col_w = column_width(&skin, &settings);
    let Some(row) = ui
        .history
        .with_untracked(|rows| rows.iter().find(|r| r.key == key).cloned())
    else {
        return widget(Stack::row(0.0)).into_any();
    };
    let inline = h.layout == HistoryLayout::Inline;
    // Echo-Live's log names every line; bubble lists group.
    let follows = follows && !inline;
    let show_avatar = settings.history.show_avatar && h.avatar.shape != AvatarShape::None;
    let size = h.avatar.size;
    let radius = match h.avatar.shape {
        AvatarShape::Circle => size / 2.0,
        _ => h.avatar.radius.unwrap_or(size * 0.18),
    };
    // Chat rows name a saved picture (`blob:…`); the local speaker's avatar
    // is a path or a URL.
    let avatar_url = row.item.avatar.as_ref().and_then(|p| {
        if let Some(url) = ui.asset_urls.with_untracked(|u| u.get(p).cloned()) {
            Some(url)
        } else if p.contains("://") {
            Some(p.clone())
        } else {
            crate::services::media::url(std::path::Path::new(p), None)
        }
    });
    let chat = row.item.kind != SourceKind::Typewriter;
    // Chat rows are drawn a little lighter than the streamer's lines.
    let skin = if chat {
        let mut s = (*skin).clone();
        let a = f32::from(s.history.bubble_color.a) * s.history.chat.bubble_opacity.clamp(0.0, 1.0);
        s.history.bubble_color = s.history.bubble_color.with_alpha(a.round() as u8);
        Arc::new(s)
    } else {
        skin
    };
    let quote = row
        .item
        .reply
        .clone()
        .filter(|_| settings.history.show_quote && h.reply.visible)
        .map(|reply| {
            // The quote sits above the bubble, on the panel (if any).
            let backdrop = h
                .panel
                .as_ref()
                .and_then(|p| super::quote::backdrop(p.color, false));
            super::quote::quote_view(
                ui,
                &skin,
                &h.reply,
                &reply,
                h.text.size,
                h.text.color,
                backdrop,
            )
        });
    let viewer = chat.then(|| viewer_color(&row.item));
    // A picture-less avatar: the viewer's colour, the skin's fill, or the
    // accent as a gradient for the streamer.
    let accent = skin.chrome.accent;
    let (fill, fill_gradient) = match (viewer, h.avatar.color) {
        (Some(v), _) => (v, None),
        (None, Some(col)) => (skin.color(col), None),
        (None, None) => (accent, Some(avatar_gradient(accent, lighten(accent, 0.35)))),
    };
    let ring = h
        .avatar
        .ring_color
        .filter(|_| h.avatar.ring_width > 0.0)
        .map(|col| (h.avatar.ring_width, c(skin.color(col))));
    let mut initial_style = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut initial_style.layout);
        l.font_size = Some(size * 0.45);
        l.font_weight = Some(700);
        l.color = Some(c(if viewer.is_some() {
            VIEWER_INK
        } else {
            crate::skin_theme::on(fill)
        }));
    }
    let avatar: AnyView = if show_avatar && follows {
        // The group's later rows keep the slot, so bubbles line up.
        widget(
            Stack::row(0.0)
                .width(LengthSpec::Px(size))
                .with_layout(|l| l.flex_shrink = Some(0.0)),
        )
        .into_any()
    } else if show_avatar {
        // The initial on its colour, with the picture laid over it: a
        // picture still loading (or unreadable) leaves the initial showing.
        let url = avatar_url.clone();
        widget(
            Stack::row(0.0)
                .width(LengthSpec::Px(size))
                .height(LengthSpec::Px(size))
                .align(AlignSpec::Center)
                .justify(nana_ui_core::JustifySpec::Center)
                .with_layout(move |l| {
                    l.position = PositionSpec::Relative;
                    l.border_radius = Some(radius);
                    l.flex_shrink = Some(0.0);
                    if let Some((w, col)) = ring {
                        l.border_width = Some(w);
                        l.border_color = Some(col);
                        l.border_style = Some(nana_ui_core::BorderStyle::Solid);
                    }
                    l.overflow_x = OverflowSpec::Hidden;
                    l.overflow_y = OverflowSpec::Hidden;
                    l.background = Some(c(fill));
                    l.paint.background_image = fill_gradient;
                }),
        )
        .children((
            widget(
                Text::new(initial(&row.item.author))
                    .decorative()
                    .style(initial_style),
            ),
            url.map(|u| super::danmaku::picture_over(u, radius)),
        ))
        .into_any()
    } else {
        widget(Stack::row(0.0)).into_any()
    };

    let mut name_style = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut name_style.layout);
        l.font_size = Some(h.name_size);
        l.font_weight = Some(h.name_weight);
        l.color = Some(c(h.name_color));
        if let Some(p) = &h.name_plate {
            l.color = Some(c(skin.color(p.text_color)));
            if viewer.is_some() {
                l.color = Some(c(VIEWER_INK));
            }
        }
        if let Some(stroke) = &h.name_stroke {
            surface::text_stroke(l, stroke);
        }
    }
    let mut name_plate = Surface::history_name_plate(&skin);
    if let (Some(plate), Some(v)) = (&mut name_plate, viewer) {
        plate.fill = Some(c(v));
        plate.gradient = None;
    }
    let plate_pad = h.name_plate.as_ref().map_or([0.0; 2], |p| p.padding);
    let mut time_style = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut time_style.layout);
        l.font_size = Some(time_size(&h));
        l.color = Some(c(h.time_color));
        l.white_space_nowrap = true;
        super::tnum(l);
    }
    let show_name = settings.history.show_name && !row.item.author.is_empty() && !follows;
    let show_time = settings.history.show_time && !follows;
    // Who is talking, once live chat shares the list.
    let role = (show_name && !inline && (chat || settings.history.chat.enabled)).then(|| {
        let key = if chat {
            "history-role-viewer"
        } else {
            "history-role-host"
        };
        widget(Text::new(ui.now_t(key)).style(time_style.clone()))
    });
    let name = show_name.then(|| {
        let plate = name_plate.clone();
        widget(
            Stack::row(0.0)
                .padding_xy(plate_pad[0], plate_pad[1])
                .with_layout(move |l| {
                    if let Some(p) = &plate {
                        p.apply(l);
                    }
                }),
        )
        .children(widget(Text::new(row.item.author.clone()).style(name_style)))
    });
    let time = show_time
        .then(|| widget(Text::new(local_time(row.item.at, h.time_seconds)).style(time_style)));

    let pad = h.bubble_padding;
    let frame_url = h.bubble_frame.as_ref().and_then(|f| {
        ui.asset_urls
            .with_untracked(|m| m.get(&format!("skin:{}", f.image)).cloned())
    });
    let mut bubble_surface = Surface::bubble(&skin, frame_url);
    if follows {
        // Only a group's first bubble points at the avatar.
        bubble_surface.radii[0] = bubble_surface.radii[0].max(h.bubble_radius);
    }
    let body = dynamic(
        move || {
            ui.history.with(|rows| {
                rows.iter().find(|r| r.key == key).map(|r| {
                    (
                        r.reveal.is_some(),
                        r.reveal.as_ref().map(|x| {
                            (
                                x.shown,
                                x.hidden_from,
                                x.segment.as_ref().map(|s| s.origin_ms),
                            )
                        }),
                    )
                })
            })
        },
        move |_| {
            let Some(row) = ui
                .history
                .with_untracked(|rows| rows.iter().find(|r| r.key == key).cloned())
            else {
                return widget(Stack::row(0.0)).into_any();
            };
            let skin = ui.skin.get_untracked();
            let entrance = Entrance {
                kind: skin.dialogue.reveal.entrance.kind,
                ms: skin.dialogue.reveal.entrance.ms,
            };
            let mut base = Base::history(&skin, entrance, ui.reduced_motion.get_untracked());
            if row.item.kind != SourceKind::Typewriter {
                let scale = skin.history.chat.text_scale;
                if scale.is_finite() && scale > 0.0 {
                    base.size *= scale.clamp(0.3, 3.0);
                }
            }
            let timing = match &row.reveal {
                Some(r) => Timing::Reveal(r.clone()),
                None => Timing::Static,
            };
            document(ui, &row.doc, &base, timing)
        },
    );
    // Inline rows share the column with the name and the time; the time's
    // width is estimated from its digits.
    let bubble_max = if inline {
        let name_w = if show_name { h.name_width + 10.0 } else { 0.0 };
        let digits = if h.time_seconds { 4.6 } else { 2.9 };
        let time_w = if show_time {
            time_size(&h) * digits + 10.0
        } else {
            0.0
        };
        (col_w - name_w - time_w).max(40.0)
    } else {
        col_w
    };
    let bubble = widget(
        Stack::column(0.0)
            .width(LengthSpec::Shrink)
            .padding_xy(pad[0], pad[1])
            .with_layout(move |l| {
                l.max_width = Some(LengthSpec::Px(bubble_max));
                bubble_surface.apply(l);
            }),
    )
    .children(body);
    // Inline rows put the quote on its own line above the whole row (over
    // the text column), so the name, the first line and the time keep
    // sharing a baseline; stacked rows put it straight above the bubble.
    let (quote, inline_quote) = if inline { (None, quote) } else { (quote, None) };
    let bubble = match quote {
        Some(q) => widget(
            Stack::column(2.0)
                .width(LengthSpec::Shrink)
                .with_layout(move |l| {
                    l.max_width = Some(LengthSpec::Px(bubble_max));
                    l.align_items = AlignSpec::Start;
                }),
        )
        .children((q, bubble))
        .into_any(),
        None => bubble.into_any(),
    };

    let content = if inline {
        let name_w = h.name_width;
        let quote_indent = if show_name { name_w + 10.0 } else { 0.0 };
        let quote_w = (col_w - quote_indent).max(40.0);
        let quote_line = inline_quote.map(|q| {
            widget(
                Stack::column(0.0)
                    .width(LengthSpec::Px(quote_w))
                    .with_layout(move |l| {
                        l.margin_left = Some(LengthSpec::Px(quote_indent));
                        l.min_width = Some(LengthSpec::Px(0.0));
                        l.flex_shrink = Some(0.0);
                    }),
            )
            .children(q)
        });
        let line = widget(
            Stack::row(10.0)
                .width(LengthSpec::Px(col_w))
                .with_layout(|l| {
                    l.min_width = Some(LengthSpec::Px(0.0));
                    // Name, first line and time share a baseline.
                    l.align_items = AlignSpec::Baseline;
                }),
        )
        .children((
            name.map(|n| {
                widget(
                    Stack::row(0.0)
                        .width(LengthSpec::Px(name_w))
                        .with_layout(|l| l.flex_shrink = Some(0.0)),
                )
                .children(n)
            }),
            // Takes the spare width, so the time sits at the right edge.
            widget(Stack::row(0.0).with_layout(|l| {
                l.flex_grow = Some(1.0);
                l.flex_shrink = Some(1.0);
                l.min_width = Some(LengthSpec::Px(0.0));
            }))
            .children(bubble),
            time.map(|t| {
                widget(Stack::row(0.0).with_layout(|l| {
                    l.flex_shrink = Some(0.0);
                }))
                .children(t)
            }),
        ));
        match quote_line {
            Some(q) => widget(
                Stack::column(2.0)
                    .width(LengthSpec::Px(col_w))
                    .with_layout(|l| {
                        l.min_width = Some(LengthSpec::Px(0.0));
                        l.align_items = AlignSpec::Start;
                    }),
            )
            .children((q, line))
            .into_any(),
            None => line.into_any(),
        }
    } else {
        let header = (name.is_some() || time.is_some())
            .then(|| widget(Stack::row(6.0).align(AlignSpec::Center)).children((name, role, time)));
        widget(
            Stack::column(6.0)
                .width(LengthSpec::Px(col_w))
                .with_layout(|l| {
                    l.min_width = Some(LengthSpec::Px(0.0));
                    l.align_items = AlignSpec::Start;
                }),
        )
        .children((header, bubble))
        .into_any()
    };

    let fade_after = settings.history.fade_after_s;
    let at = row.item.at;
    // A group's later rows sit closer to the one above.
    let pull = if follows { GROUP_GAP - h.gap } else { 0.0 };
    widget(
        Stack::row(10.0)
            .align(AlignSpec::Start)
            .width(LengthSpec::Fill)
            .with_layout(move |l| {
                if pull != 0.0 {
                    l.margin_top = Some(LengthSpec::Px(pull));
                }
            }),
    )
    .bind(move |s: &mut Stack| {
        let Some(after) = fade_after else {
            return;
        };
        let age = crate::clock::unix_of(ui.now.get()).saturating_sub(at);
        let start = u64::from(after) * 1000;
        let opacity = 1.0 - (age.saturating_sub(start) as f32 / 600.0).clamp(0.0, 1.0);
        let mut taken = std::mem::replace(s, Stack::row(0.0));
        taken = taken.with_layout(|l| l.opacity = Some(opacity));
        *s = taken;
    })
    // Inline rows drop the empty avatar slot, so the name sits at the edge.
    .children(((show_avatar || !inline).then_some(avatar), content))
    .into_any()
}

/// The newest `visible` rows, leaving out the line `on_screen`.
fn shown_keys(
    rows: impl Iterator<Item = (u64, Option<LineId>)>,
    visible: usize,
    on_screen: Option<LineId>,
) -> Vec<u64> {
    let keys: Vec<u64> = rows
        .filter(|(_, line)| on_screen.is_none() || *line != on_screen)
        .map(|(key, _)| key)
        .collect();
    let start = keys.len().saturating_sub(visible);
    keys[start..].to_vec()
}

pub fn history_window(ui: Ui) -> impl IntoView {
    let items = computed(move || {
        let (visible, hide) = ui
            .settings
            .with(|s| (s.history.visible_items as usize, s.history.hide_latest));
        let hide = hide.resolve(ui.skin.with(|k| k.history.hide_latest));
        // The line still on the dialogue box joins once the box clears or
        // the next line replaces it.
        let on_screen = if hide {
            ui.display.with(|d| d.line)
        } else {
            None
        };
        ui.history.with(|rows| {
            shown_keys(
                rows.iter().map(|r| (r.key, r.item.line)),
                visible,
                on_screen,
            )
        })
    });
    let follows = computed(move || {
        let shown = items.get();
        ui.history.with(|rows| follow_ups(rows, &shown))
    });
    let key = move || {
        let skin = ui.skin.get();
        let settings = ui.settings.get();
        (Arc::as_ptr(&skin) as usize, Arc::as_ptr(&settings) as usize)
    };
    dynamic(key, move |_| {
        let skin = ui.skin.get_untracked();
        let gap = skin.history.gap;
        let ms = std::time::Duration::from_millis(ROW_ENTER_MS);
        // Reduced motion keeps the fade and drops the travel.
        let reduced = ui.reduced_motion.get_untracked();
        let transition = match skin.history.entrance {
            HistoryEntrance::None => None,
            _ if reduced => Some(Transition::fade(ms)),
            HistoryEntrance::SlideUp => Some(Transition::slide(0.0, 16.0, ms)),
            HistoryEntrance::SlideLeft => Some(Transition::slide(-24.0, 0.0, ms)),
            HistoryEntrance::Fade => Some(Transition::fade(ms)),
        }
        .map(|t| t.ease(Easing::CubicBezier([0.2, 0.8, 0.2, 1.0])));
        let list = each(
            items,
            |k: &u64| *k,
            move |k: u64| {
                dynamic(
                    move || follows.with(|f| f.contains(&k)),
                    move |f| row_view(ui, k, *f),
                )
            },
        );
        let list = match transition {
            Some(t) => list.transition(t),
            None => list,
        };
        let h = &skin.history;
        let [mt, mr, mb, ml] = h.panel.as_ref().map_or([8.0; 4], |p| p.margin);
        let [pl, pt, pr, pb] = h.panel.as_ref().map_or([4.0; 4], |p| p.padding);
        let panel_surface = Surface::history_panel(&skin);
        let decor = |layer: DecorLayer| -> Vec<AnyView> {
            h.decor
                .iter()
                .filter(|d| d.layer == layer)
                .map(|d| dialogue::decor(&skin, d, None::<fn() -> bool>))
                .collect()
        };
        let (below, above) = (decor(DecorLayer::Below), decor(DecorLayer::Above));
        let title = h.title.as_ref().map(|t| title_view(&skin, t));
        let justify = match h.anchor {
            HistoryAnchor::Bottom => nana_ui_core::JustifySpec::End,
            HistoryAnchor::Top => nana_ui_core::JustifySpec::Start,
        };
        widget(Stack::fill_column(0.0).with_layout(move |l| {
            l.padding_top = Some(LengthSpec::Px(mt));
            l.padding_right = Some(LengthSpec::Px(mr));
            l.padding_bottom = Some(LengthSpec::Px(mb));
            l.padding_left = Some(LengthSpec::Px(ml));
        }))
        .bind(move |s: &mut Stack| {
            let editing = ui
                .settings
                .with(|x| x.overlays.history.lock == nanatype_settings::Lock::EditLayout);
            let accent = c(ui.skin.with(|k| k.chrome.accent));
            let mut taken = std::mem::replace(s, Stack::row(0.0));
            taken = taken.with_layout(|l| super::dialogue::edit_outline(l, editing, accent));
            *s = taken;
        })
        .children(
            widget(Stack::fill_column(0.0).with_layout(move |l| {
                l.position = PositionSpec::Relative;
                if let Some(s) = &panel_surface {
                    s.apply(l);
                }
                // The list would scroll under the press. Layout mode moves
                // the window; this view is rebuilt when the lock changes.
                if ui.settings.with_untracked(|x| {
                    x.overlays.history.lock == nanatype_settings::Lock::EditLayout
                }) {
                    l.pointer_events = Some(PointerEventsSpec::None);
                }
            }))
            .children((
                below,
                title,
                // Takes the panel's height left under the title (and no
                // more), so the list scrolls inside it instead of running
                // past the window's foot.
                widget(
                    ScrollView::new(ScrollAxes::Vertical)
                        .follow_end(true)
                        .with_layout(|l| {
                            l.flex_grow = Some(1.0);
                            l.flex_shrink = Some(1.0);
                            l.min_height = Some(LengthSpec::Px(0.0));
                        }),
                )
                .children(
                    widget(Stack::column(gap).with_layout(move |l| {
                        l.padding_left = Some(LengthSpec::Px(pl));
                        l.padding_top = Some(LengthSpec::Px(pt));
                        l.padding_right = Some(LengthSpec::Px(pr));
                        l.padding_bottom = Some(LengthSpec::Px(pb));
                        // At least the viewport tall, so `justify_content`
                        // has spare room to anchor the rows in.
                        l.min_height = Some(LengthSpec::Percent(100.0));
                        l.justify_content = justify;
                    }))
                    .children(list),
                ),
                above,
            )),
        )
    })
}

/// The header bar across the top of the panel.
fn title_view(skin: &Skin, t: &HistoryTitle) -> AnyView {
    let mut st = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.font_size = Some(t.size);
        l.font_weight = Some(t.weight);
        l.font_family = skin.fonts.name.families.first().cloned();
        l.color = Some(c(t.color));
        l.white_space_nowrap = true;
    }
    let background = c(t.background);
    widget(
        Stack::row(0.0)
            .width(LengthSpec::Fill)
            .height(LengthSpec::Px(t.height))
            .padding_xy(t.padding, 0.0)
            .align(AlignSpec::Center)
            .with_layout(move |l| {
                l.background = Some(background);
                l.flex_shrink = Some(0.0);
            }),
    )
    .children(widget(Text::new(t.text.clone()).style(st)))
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_on_screen_waits_until_it_leaves() {
        let rows = [(1, Some(LineId(10))), (2, None), (3, Some(LineId(11)))];
        let all = || rows.iter().copied();
        assert_eq!(shown_keys(all(), 30, None), [1, 2, 3]);
        // Only the line still on the box is left out; external rows stay.
        assert_eq!(shown_keys(all(), 30, Some(LineId(11))), [1, 2]);
        // The cap counts the rows that show.
        assert_eq!(shown_keys(all(), 1, Some(LineId(11))), [2]);
    }

    fn item(author: &str, kind: SourceKind, at_s: u64) -> HistoryItem {
        HistoryItem {
            source: nanatype_model::SourceId::local(),
            kind,
            line: None,
            author: author.into(),
            avatar: None,
            doc: nanatype_model::Document::new(),
            at: at_s * 1000,
            uid: None,
            reply: None,
        }
    }

    #[test]
    fn rows_of_one_speaker_close_together_group() {
        use SourceKind::{Chat, Typewriter};
        let rows: Vec<HistoryRow> = [
            item("Nana", Typewriter, 0),
            item("Nana", Typewriter, 30),
            item("Nana", Chat, 40),
            item("Nana", Typewriter, 50),
            item("Nana", Typewriter, 200),
            item("Mio", Typewriter, 210),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, item)| HistoryRow {
            key: i as u64,
            item,
            doc: Arc::new(nanatype_model::Document::new()),
            reveal: None,
        })
        .collect();
        let shown: Vec<u64> = (0..6).collect();
        // A chat row with the streamer's name, a two-minute pause and a new
        // speaker each start a group.
        assert_eq!(follow_ups(&rows, &shown), [1]);
        // The first shown row always has its header.
        assert_eq!(follow_ups(&rows, &shown[1..]), Vec::<u64>::new());
    }

    #[test]
    fn a_viewer_keeps_their_colour() {
        let a = item("路过的猫", SourceKind::Chat, 0);
        assert_eq!(viewer_color(&a), viewer_color(&a.clone()));
        let mut b = a.clone();
        b.uid = Some("1002".into());
        assert!(VIEWER_COLORS.contains(&viewer_color(&b)));
    }
}
