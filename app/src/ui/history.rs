//! The history window: a blivechat-style column of avatar, name and bubble.
//! Rows are drawn by the same document builder as the dialogue box, so a line
//! keeps its colours, effects and stickers; with "typing along" sync the
//! newest row reveals in step with the typewriter.

use std::sync::Arc;

use nana_ui::runtime::view::{AnyView, IntoView, Transition, computed, dynamic, each, widget};
use nana_ui::runtime::{NodeStyle, ScrollAxes, ScrollView, Stack, Text};
use nana_ui_core::{
    AlignSpec, BackgroundImage, BackgroundImageFit, LengthSpec, OverflowSpec, PositionSpec,
};
use nanatype_model::{Entrance, LineId, SourceKind};
use nanatype_theme::{
    AvatarShape, DecorLayer, HistoryAnchor, HistoryEntrance, HistoryLayout, HistoryTitle, Skin,
};

use super::dialogue::{self, Base, Timing, document};
use super::surface::{self, Surface};
use super::{Ui, c};
use crate::assets::file_url;

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

fn time_size(h: &nanatype_theme::History) -> f32 {
    h.time_size.unwrap_or((h.name_size - 2.0).max(9.0))
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

fn row_view(ui: Ui, key: u64) -> AnyView {
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
    let show_avatar = settings.history.show_avatar && h.avatar.shape != AvatarShape::None;
    let size = h.avatar.size;
    let radius = match h.avatar.shape {
        AvatarShape::Circle => size / 2.0,
        _ => size * 0.18,
    };
    // Chat rows name a saved picture (`blob:…`); the local speaker's avatar
    // is a path or a URL.
    let avatar_url = row.item.avatar.as_ref().map(|p| {
        if let Some(url) = ui.asset_urls.with_untracked(|u| u.get(p).cloned()) {
            url
        } else if p.contains("://") {
            p.clone()
        } else {
            file_url(std::path::Path::new(p))
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
            super::quote::quote_view(ui, &skin, &h.reply, &reply, h.text.size, h.text.color)
        });
    let accent = c(skin.chrome.accent);
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
        l.color = Some(c(crate::skin_theme::on(skin.chrome.accent)));
    }
    let avatar: AnyView = if show_avatar {
        let has_url = avatar_url.is_some();
        let url = avatar_url.clone();
        let el = widget(
            Stack::row(0.0)
                .width(LengthSpec::Px(size))
                .height(LengthSpec::Px(size))
                .align(AlignSpec::Center)
                .justify(nana_ui_core::JustifySpec::Center)
                .with_layout(move |l| {
                    l.border_radius = Some(radius);
                    l.flex_shrink = Some(0.0);
                    if let Some((w, col)) = ring {
                        l.border_width = Some(w);
                        l.border_color = Some(col);
                        l.border_style = Some(nana_ui_core::BorderStyle::Solid);
                    }
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
            el.into_any()
        } else {
            el.children(widget(
                Text::new(initial(&row.item.author))
                    .decorative()
                    .style(initial_style),
            ))
            .into_any()
        }
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
        }
        if let Some(stroke) = &h.name_stroke {
            surface::text_stroke(l, stroke);
        }
    }
    let name_plate = Surface::history_name_plate(&skin);
    let plate_pad = h.name_plate.as_ref().map_or([0.0; 2], |p| p.padding);
    let mut time_style = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut time_style.layout);
        l.font_size = Some(time_size(&h));
        l.color = Some(c(h.time_color));
    }
    let inline = h.layout == HistoryLayout::Inline;
    let show_name = settings.history.show_name && !row.item.author.is_empty();
    let show_time = settings.history.show_time;
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
    let bubble_surface = Surface::bubble(&skin, frame_url);
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
        widget(
            Stack::row(10.0)
                .width(LengthSpec::Px(col_w))
                .with_layout(|l| {
                    l.min_width = Some(LengthSpec::Px(0.0));
                    l.align_items = AlignSpec::Start;
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
            // On the last line of a wrapped message.
            time.map(|t| {
                widget(Stack::row(0.0).with_layout(|l| {
                    l.align_self = Some(AlignSpec::End);
                    l.flex_shrink = Some(0.0);
                }))
                .children(t)
            }),
        ))
        .into_any()
    } else {
        widget(
            Stack::column(4.0)
                .width(LengthSpec::Px(col_w))
                .with_layout(|l| {
                    l.min_width = Some(LengthSpec::Px(0.0));
                    l.align_items = AlignSpec::Start;
                }),
        )
        .children((
            widget(Stack::row(6.0).align(AlignSpec::Center)).children((name, time)),
            bubble,
        ))
        .into_any()
    };

    let fade_after = settings.history.fade_after_s;
    let at = row.item.at;
    widget(
        Stack::row(10.0)
            .align(AlignSpec::Start)
            .width(LengthSpec::Fill),
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
    let key = move || {
        let skin = ui.skin.get();
        let settings = ui.settings.get();
        (Arc::as_ptr(&skin) as usize, Arc::as_ptr(&settings) as usize)
    };
    dynamic(key, move |_| {
        let skin = ui.skin.get_untracked();
        let gap = skin.history.gap;
        let transition = match skin.history.entrance {
            HistoryEntrance::None => None,
            HistoryEntrance::SlideUp => Some(Transition::slide(
                0.0,
                16.0,
                std::time::Duration::from_millis(220),
            )),
            HistoryEntrance::SlideLeft => Some(Transition::slide(
                -24.0,
                0.0,
                std::time::Duration::from_millis(220),
            )),
            HistoryEntrance::Fade => Some(Transition::fade(std::time::Duration::from_millis(220))),
        };
        let list = each(items, |k: &u64| *k, move |k: u64| row_view(ui, k));
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
            }))
            .children((
                below,
                title,
                widget(ScrollView::new(ScrollAxes::Vertical).follow_end(true)).children(
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
}
