//! The history window: a blivechat-style column of avatar, name and bubble.
//! Rows are drawn by the same document builder as the dialogue box, so a line
//! keeps its colours, effects and stickers; with "typing along" sync the
//! newest row reveals in step with the typewriter.

use std::sync::Arc;

use nana_ui::runtime::view::{AnyView, IntoView, Transition, computed, dynamic, each, widget};
use nana_ui::runtime::{NodeStyle, ScrollAxes, ScrollView, Stack, Text};
use nana_ui_core::{
    AlignSpec, BackgroundImage, BackgroundImageFit, LengthSpec, OverflowSpec, TextShadowSpec,
};
use nanatype_model::Entrance;
use nanatype_theme::{AvatarShape, HistoryEntrance};

use super::dialogue::{Base, Timing, document};
use super::{Ui, c};
use crate::assets::file_url;

pub fn local_hm(unix_ms: u64) -> String {
    let offset = crate::localtime::offset_seconds();
    let secs = (unix_ms / 1000) as i64 + offset;
    let day = secs.rem_euclid(86_400);
    format!("{:02}:{:02}", day / 3600, (day % 3600) / 60)
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
    (w - 16.0 - 8.0 - avatar).max(80.0)
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
    let avatar_url = row.item.avatar.as_ref().map(|p| {
        if p.contains("://") {
            p.clone()
        } else {
            file_url(std::path::Path::new(p))
        }
    });
    let accent = c(skin.chrome.accent);
    let mut initial_style = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut initial_style.layout);
        l.font_size = Some(size * 0.45);
        l.font_weight = Some(700);
        l.color = Some([1.0, 1.0, 1.0, 1.0]);
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
        l.font_weight = Some(700);
        l.color = Some(c(h.name_color));
        if let Some(stroke) = &h.name_stroke {
            l.paint.text_shadow = Some(TextShadowSpec {
                offset_x: 0.0,
                offset_y: 0.0,
                blur_radius: stroke.width * 1.5,
                color: c(stroke.color),
                paint_color: None,
            });
        }
    }
    let mut time_style = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut time_style.layout);
        l.font_size = Some((h.name_size - 2.0).max(9.0));
        l.color = Some(c(h.time_color));
    }
    let show_name = settings.history.show_name && !row.item.author.is_empty();
    let show_time = settings.history.show_time;
    let header = widget(Stack::row(6.0).align(AlignSpec::Center)).children((
        show_name.then(|| widget(Text::new(row.item.author.clone()).style(name_style))),
        show_time.then(|| widget(Text::new(local_hm(row.item.at)).style(time_style))),
    ));

    let bubble_color = c(h.bubble_color);
    let bubble_border = h.bubble_border.map(c);
    let radius = h.bubble_radius;
    let pad = h.bubble_padding;
    let frame = h.bubble_frame.clone();
    let frame_url = frame.as_ref().and_then(|f| {
        ui.asset_urls
            .with_untracked(|m| m.get(&format!("skin:{}", f.image)).cloned())
    });
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
            let base = Base::history(&skin, entrance, ui.reduced_motion.get_untracked());
            let timing = match &row.reveal {
                Some(r) => Timing::Reveal(r.clone()),
                None => Timing::Static,
            };
            document(ui, &row.doc, &base, timing)
        },
    );
    let bubble = widget(
        Stack::column(0.0)
            .width(LengthSpec::Shrink)
            .padding_xy(pad[0], pad[1])
            .with_layout(move |l| {
                l.max_width = Some(LengthSpec::Px(col_w));
                match (&frame, frame_url) {
                    (Some(f), Some(url)) => {
                        let mut spec =
                            nana_ui_core::BorderImageSpec::from_source(BackgroundImage::url(url));
                        spec.slice = f
                            .slice
                            .map(|v| nana_ui_core::BorderImageSlice::Number(v as f32));
                        spec.fill = true;
                        l.paint.border_image = Some(spec);
                    }
                    _ => {
                        l.background = Some(bubble_color);
                        l.border_radius = Some(radius);
                        if let Some(b) = bubble_border {
                            l.border_width = Some(1.0);
                            l.border_color = Some(b);
                            l.border_style = Some(nana_ui_core::BorderStyle::Solid);
                        }
                    }
                }
            }),
    )
    .children(body);

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
    .children((
        avatar,
        widget(
            Stack::column(4.0)
                .width(LengthSpec::Px(col_w))
                .with_layout(|l| {
                    l.min_width = Some(LengthSpec::Px(0.0));
                    l.align_items = AlignSpec::Start;
                }),
        )
        .children((header, bubble)),
    ))
    .into_any()
}

pub fn history_window(ui: Ui) -> impl IntoView {
    let items = computed(move || {
        let visible = ui.settings.with(|s| s.history.visible_items) as usize;
        ui.history.with(|rows| {
            let start = rows.len().saturating_sub(visible);
            rows[start..].iter().map(|r| r.key).collect::<Vec<u64>>()
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
        widget(Stack::fill_column(0.0).padding(8.0))
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
                widget(ScrollView::new(ScrollAxes::Vertical).follow_end(true)).children(
                    widget(Stack::column(gap).padding_xy(4.0, 4.0).with_layout(|l| {
                        l.justify_content = nana_ui_core::JustifySpec::End;
                    }))
                    .children(list),
                ),
            )
    })
}
