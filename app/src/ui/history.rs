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
use nanatype_model::{Document, Entrance, HistoryItem, Inline, LineId, Rgba8, SourceKind, Stroke};
use nanatype_theme::{AvatarShape, DecorLayer, HistoryEntrance, HistoryLayout, HistoryTitle, Skin};

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

/// Keep enough room for a short phrase and an @ label in narrow inline logs.
/// At the skin's normal width this leaves its requested name column unchanged.
fn inline_name_width(h: &nanatype_theme::History, col_w: f32, show_time: bool) -> f32 {
    let time = if show_time {
        time_size(h) * if h.time_seconds { 4.6 } else { 2.9 } + 10.0
    } else {
        0.0
    };
    let body = (h.text.size * 6.0).max(80.0);
    h.name_width.min((col_w - time - body - 10.0).max(32.0))
}

/// Outline only text runs, so @ labels retain their own fill and lettering.
fn transparent_text(doc: &Document, color: Rgba8) -> Document {
    let mut drawn = doc.clone();
    for paragraph in &mut drawn.paragraphs {
        for inline in &mut paragraph.inlines {
            let run = match inline {
                Inline::Text(run) => run,
                Inline::Ruby(ruby) => &mut ruby.base,
                _ => continue,
            };
            if run.style.stroke.is_none() {
                run.style.stroke = Some(Stroke {
                    color: crate::skin_theme::on(run.style.color.unwrap_or(color)),
                    width_px: 1.5,
                });
            }
        }
    }
    drawn
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
const GROUP_GAP: f32 = 8.0;

/// Positive spacing between entries: grouping never pulls one bubble into another.
fn row_gap(skin: &Skin, follows: bool) -> f32 {
    let h = &skin.history;
    if h.layout == HistoryLayout::Inline {
        h.gap.clamp(8.0, 14.0)
    } else if follows {
        (h.text.size * 0.45).round().clamp(GROUP_GAP, 12.0)
    } else {
        h.gap.max((h.text.size * 0.65).round()).clamp(12.0, 20.0)
    }
}

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

fn find_row(ui: Ui, key: u64) -> Option<HistoryRow> {
    ui.history
        .with_untracked(|rows| rows.iter().find(|r| r.key == key).cloned())
}

/// Halo's presentation follows the approved interaction preview. Other skins
/// retain their own log layout and speaker grouping. All histories start at the top.
fn interaction_presentation(skin: &Skin) -> bool {
    skin.meta.id == "nanatype.halo"
}

fn entry_view(ui: Ui, key: u64, first: bool, follows: bool) -> AnyView {
    let gap = if first {
        0.0
    } else {
        ui.skin.with_untracked(|s| row_gap(s, follows))
    };
    widget(
        Stack::column(0.0)
            .width(LengthSpec::Fill)
            .with_layout(move |l| {
                l.margin_top = Some(LengthSpec::Px(gap));
            }),
    )
    .children(row_view(ui, key, follows))
    .into_any()
}

fn row_view(ui: Ui, key: u64, follows: bool) -> AnyView {
    let mut skin = ui.skin.get_untracked();
    let settings = ui.settings.get_untracked();
    let fitted = interaction_presentation(&skin);
    if fitted {
        let k = Arc::make_mut(&mut skin);
        k.history.avatar.size = k.history.avatar.size.min(36.0);
        k.history.text.size = k.history.text.size.clamp(14.0, 17.0);
    }
    let h = skin.history.clone();
    let exposed = h
        .panel
        .as_ref()
        .and_then(|p| super::quote::backdrop(p.color, false))
        .is_none();
    let col_w = if fitted {
        let (width, _) =
            crate::windows::size_of(crate::windows::WindowKind::History, &skin, &settings);
        (width
            - 38.0
            - if !settings.history.show_avatar || h.avatar.shape == AvatarShape::None {
                0.0
            } else {
                h.avatar.size + 10.0
            })
        .max(60.0)
    } else {
        column_width(&skin, &settings)
    };
    let Some(row) = find_row(ui, key) else {
        return widget(Stack::row(0.0)).into_any();
    };
    let inline = h.layout == HistoryLayout::Inline && !fitted;
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
            super::quote::quote_view(ui, &skin, &h.reply, &reply, &h.text, backdrop)
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
        let asset = row.item.avatar.clone();
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
            Some(super::danmaku::reactive_picture(ui, asset, radius)),
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
        l.font_family = skin.fonts.name.families.first().cloned();
        l.color = Some(c(h.name_color));
        if let Some(p) = &h.name_plate {
            l.color = Some(c(skin.color(p.text_color)));
            if viewer.is_some() {
                l.color = Some(c(VIEWER_INK));
            }
        }
        if let Some(stroke) = &h.name_stroke {
            surface::text_stroke(l, stroke);
        } else if exposed && h.name_plate.is_none() {
            surface::text_stroke(
                l,
                &nanatype_theme::TextStrokeSpec {
                    color: crate::skin_theme::on(h.name_color),
                    width: 1.0,
                },
            );
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
        l.font_family = skin.fonts.body.families.first().cloned();
        l.color = Some(c(h.time_color));
        if exposed {
            surface::text_stroke(
                l,
                &nanatype_theme::TextStrokeSpec {
                    color: crate::skin_theme::on(h.time_color),
                    width: 1.0,
                },
            );
        }
        l.white_space_nowrap = true;
        super::tnum(l);
    }
    let show_name = settings.history.show_name && !row.item.author.is_empty() && !follows;
    let show_time = settings.history.show_time && !follows;
    let inline_name_w = inline_name_width(&h, col_w, show_time);
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
        let max_name = if inline { inline_name_w } else { col_w - 64.0 }.max(32.0);
        Arc::make_mut(&mut name_style.layout).max_width =
            Some(LengthSpec::Px((max_name - plate_pad[0] * 2.0).max(12.0)));
        widget(
            Surface::optional_stack(
                plate.as_ref(),
                Stack::row(0.0).padding_xy(plate_pad[0], plate_pad[1]),
            )
            .with_layout(move |l| {
                l.min_width = Some(LengthSpec::Px(0.0));
                l.max_width = Some(LengthSpec::Px(max_name));
                l.overflow_x = OverflowSpec::Hidden;
                l.flex_shrink = Some(1.0);
            }),
        )
        .children(widget(
            Text::new(row.item.author.clone())
                .style(name_style)
                .truncating(),
        ))
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
    let body_skin = skin.clone();
    let body = dynamic(
        move || {
            ui.history.with(|_| ());
            find_row(ui, key).map(|r| {
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
        },
        move |_| {
            let Some(row) = find_row(ui, key) else {
                return widget(Stack::row(0.0)).into_any();
            };
            let skin = body_skin.clone();
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
            if exposed && skin.history.bubble_color.a < 128 && base.stroke.is_none() {
                document(ui, &transparent_text(&row.doc, base.color), &base, timing)
            } else {
                document(ui, &row.doc, &base, timing)
            }
        },
    );
    // Inline rows share the column with the name and the time; the time's
    // width is estimated from its digits.
    let bubble_max = if inline {
        let name_w = if show_name { inline_name_w + 10.0 } else { 0.0 };
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
        bubble_surface
            .stack(
                Stack::column(0.0)
                    .width(LengthSpec::Shrink)
                    .padding_xy(pad[0], pad[1]),
            )
            .with_layout(move |l| {
                l.max_width = Some(LengthSpec::Px(bubble_max));
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

    let content =
        if inline {
            let name_w = inline_name_w;
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
                Some(q) => widget(Stack::column(2.0).width(LengthSpec::Px(col_w)).with_layout(
                    |l| {
                        l.min_width = Some(LengthSpec::Px(0.0));
                        l.align_items = AlignSpec::Start;
                    },
                ))
                .children((q, line))
                .into_any(),
                None => line.into_any(),
            }
        } else {
            let header = (name.is_some() || time.is_some()).then(|| {
                widget(
                    Stack::row(6.0)
                        .align(AlignSpec::Center)
                        .width(LengthSpec::Fill),
                )
                .children((name, role, time))
                .into_any()
            });
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
    let view = widget(
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
    .into_any();
    if fitted {
        widget(
            Stack::column(0.0)
                .width(LengthSpec::Fill)
                .padding_xy(8.0, 0.0),
        )
        .children(view)
        .into_any()
    } else {
        view
    }
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
            if ui.peer.with(|p| p.active) {
                ui.peer_local_line.get()
            } else {
                ui.display.with(|d| d.line)
            }
        } else {
            None
        };
        ui.history.with(|rows| {
            let peer_on_screen = if hide && ui.peer.with(|p| p.active) {
                ui.peer_display.with(|d| d.line)
            } else {
                None
            };
            shown_keys(
                rows.iter()
                    .filter(|r| peer_on_screen.is_none() || r.item.line != peer_on_screen)
                    .map(|r| (r.key, r.item.line)),
                visible,
                on_screen,
            )
        })
    });
    let follows = computed(move || {
        if ui.skin.with(|s| interaction_presentation(s)) {
            return Vec::new();
        }
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
        let presentation = interaction_presentation(&skin);
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
                    move || {
                        (
                            items.with(|i| i.first() == Some(&k)),
                            follows.with(|f| f.contains(&k)),
                        )
                    },
                    move |(first, f)| entry_view(ui, k, *first, *f),
                )
            },
        );
        let list = match transition {
            Some(t) => list.transition(t),
            None => list,
        };
        let h = &skin.history;
        let [mt, mr, mb, ml] = h
            .panel
            .as_ref()
            .map_or(if presentation { [0.0; 4] } else { [8.0; 4] }, |p| p.margin);
        let [pl, pt, pr, pb] = h.panel.as_ref().map_or(
            if presentation {
                [6.0, 16.0, 6.0, 16.0]
            } else {
                [4.0, 8.0, 4.0, 8.0]
            },
            |p| p.padding,
        );
        let panel_surface = Surface::history_panel(&skin);
        let decor = |layer: DecorLayer| -> Vec<AnyView> {
            h.decor
                .iter()
                .filter(|d| d.layer == layer)
                .map(|d| {
                    let image = d.image.as_ref().and_then(|path| {
                        ui.asset_urls
                            .with_untracked(|urls| urls.get(&format!("skin:{path}")).cloned())
                    });
                    dialogue::decor(&skin, d, None::<fn() -> bool>, image)
                })
                .collect()
        };
        let (below, above) = (decor(DecorLayer::Below), decor(DecorLayer::Above));
        let title = h.title.as_ref().map(|t| title_view(&skin, t));
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
            widget(
                Surface::optional_stack(panel_surface.as_ref(), Stack::fill_column(0.0))
                    .with_layout(move |l| {
                        l.position = PositionSpec::Relative;
                        // The list would scroll under the press. Layout mode moves
                        // the window; this view is rebuilt when the lock changes.
                        if ui.settings.with_untracked(|x| {
                            x.overlays.history.lock == nanatype_settings::Lock::EditLayout
                        }) {
                            l.pointer_events = Some(PointerEventsSpec::None);
                        }
                    }),
            )
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
                    widget(Stack::column(0.0).with_layout(move |l| {
                        l.padding_left = Some(LengthSpec::Px(pl));
                        l.padding_top = Some(LengthSpec::Px(pt));
                        l.padding_right = Some(LengthSpec::Px(pr));
                        l.padding_bottom = Some(LengthSpec::Px(pb));
                        // A short log always starts at its content area's top;
                        // follow_end still scrolls overflowing logs to the newest row.
                        l.min_height = Some(LengthSpec::Percent(100.0));
                        l.justify_content = nana_ui_core::JustifySpec::Start;
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
