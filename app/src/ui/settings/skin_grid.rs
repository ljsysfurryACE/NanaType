//! Static thumbnails for the skin library's card grid.
//!
//! NanaUI has no product-side offscreen path (its offscreen painter lives in
//! the devtools crate, for snapshots only), so a card does not show a
//! rendered picture of the box. It shows a miniature instead: the box and
//! name plate painted from the skin's own [`Surface`]s — the same fill,
//! gradient, border, radii and shadows the live box uses — with bars for the
//! text and a dot of the accent. Portraits, decorations, tails and text
//! effects are left out.
//!
//! A miniature is worked out once per loaded skin and cached ([`thumb`]);
//! it has no text, so it does not depend on the locale. The nodes it builds
//! are still, so 18 cards cost nothing per frame.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nana_ui::runtime::Stack;
use nana_ui::runtime::view::{AnyView, IntoView, widget};
use nana_ui_core::{LengthSpec, OverflowSpec, PointerEventsSpec, PositionSpec, TransformOrigin};
use nanatype_theme::{Skin, TextAlign};

use crate::ui::surface::{Surface, affine};
use crate::ui::{SkinRow, c};

/// The space a miniature is fitted into, in px.
pub const FIT: [f32; 2] = [172.0, 60.0];
/// The widest a miniature box gets, in box heights: a 1280×180 box reads
/// as a sliver at card size, so wide boxes are drawn shorter.
const MAX_ASPECT: f32 = 3.0;

/// A placed rectangle, `[x, y, w, h]`, in the skin's own px.
type Rect = [f32; 4];

/// What a miniature draws, in the skin's px, and the scale that fits it.
#[derive(Debug, Clone, PartialEq)]
pub struct Thumb {
    /// Everything drawn: the box, the plate and a little room for shadows.
    pub size: [f32; 2],
    pub scale: f32,
    pub box_rect: Rect,
    pub box_surface: Surface,
    /// `[left, top, right, bottom]`.
    pub padding: [f32; 4],
    pub text_color: [f32; 4],
    /// The text's outline, so outlined text on no box still shows.
    pub stroke: Option<([f32; 4], f32)>,
    /// Bar height and line pitch.
    pub bar: f32,
    pub pitch: f32,
    pub lines: usize,
    pub align: TextAlign,
    pub plate: Option<Plate>,
    pub accent: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plate {
    pub rect: Rect,
    pub surface: Surface,
    pub text_color: [f32; 4],
    pub bar: [f32; 2],
}

impl Thumb {
    /// The miniature of a loaded skin. Frame images resolve against
    /// `urls` (`skin:<path>` → URL); without one the box takes its fill.
    pub fn of(skin: &Skin, urls: &HashMap<String, String>) -> Self {
        let d = &skin.dialogue;
        let url = |path: &str| urls.get(&format!("skin:{path}")).cloned();
        let [mt, mr, mb, ml] = d.box_.margin;
        let bh = (d.region[1] - mt - mb).max(24.0);
        let bw = ((d.region[0] - ml - mr) * d.box_.width_fraction).clamp(bh, bh * MAX_ASPECT);
        let box_surface =
            Surface::dialogue_box(skin, d.box_.frame.as_ref().and_then(|f| url(&f.image)));
        let size = d.text.size.max(8.0);
        let pitch = size * d.text.line_height.max(1.0);
        let [pl, pt, pr, pb] = d.padding;
        let room = (bh - pt - pb).max(pitch);
        let lines = ((room / pitch).floor() as usize).clamp(1, 2);
        let p = &d.nameplate;
        let plate = p.visible.then(|| {
            let name = p.size.max(8.0);
            let bar = [name * 2.2, name * 0.5];
            let (pw, ph) = (
                bar[0] + 2.0 * p.padding[0],
                name * 1.25 + 2.0 * p.padding[1],
            );
            let [fx, fy] = nanatype_theme::anchor_frac(p.anchor);
            let [ox, oy] = p.offset;
            Plate {
                rect: [fx * (bw - pw) + ox, fy * (bh - ph) + oy, pw, ph],
                surface: Surface::nameplate(skin, p.frame.as_ref().and_then(|f| url(&f.image))),
                text_color: c(p.text_color),
                bar,
            }
        });
        // The box and plate together, with room around them for shadows.
        let mut lo = [0.0f32, 0.0];
        let mut hi = [bw, bh];
        if let Some(p) = &plate {
            let [x, y, w, h] = p.rect;
            lo = [lo[0].min(x), lo[1].min(y)];
            hi = [hi[0].max(x + w), hi[1].max(y + h)];
        }
        let room = bh * 0.06;
        let (lo, hi) = (lo.map(|v| v - room), hi.map(|v| v + room));
        let total = [hi[0] - lo[0], hi[1] - lo[1]];
        let scale = (FIT[0] / total[0]).min(FIT[1] / total[1]).min(1.0);
        let shift = |r: Rect| [r[0] - lo[0], r[1] - lo[1], r[2], r[3]];
        Self {
            size: total,
            scale,
            box_rect: shift([0.0, 0.0, bw, bh]),
            box_surface,
            padding: [pl, pt, pr, pb],
            text_color: c(d.text.color),
            stroke: d
                .text
                .stroke
                .as_ref()
                .map(|s| (c(s.color), s.width.max(1.0))),
            bar: size * 0.5,
            pitch,
            lines,
            align: d.text.align,
            plate: plate.map(|p| Plate {
                rect: shift(p.rect),
                ..p
            }),
            accent: c(skin.chrome.accent),
        }
    }

    /// A stand-in for a skin that did not load: a plain box in the row's
    /// colours.
    pub fn fallback(row: &SkinRow) -> Self {
        let (bw, bh) = (180.0, 60.0);
        let room = 4.0;
        let size = [bw + 2.0 * room, bh + 2.0 * room];
        Self {
            size,
            scale: (FIT[0] / size[0]).min(FIT[1] / size[1]),
            box_rect: [room, room, bw, bh],
            box_surface: Surface {
                fill: Some(row.box_color),
                radii: [8.0; 4],
                ..Surface::default()
            },
            padding: [14.0, 12.0, 14.0, 12.0],
            text_color: row.text_color,
            stroke: None,
            bar: 8.0,
            pitch: 16.0,
            lines: 2,
            align: TextAlign::Start,
            plate: None,
            accent: row.accent,
        }
    }
}

/// Cached miniatures by skin id, with the skin they were made from: a
/// reloaded skin is a new `Arc` and is worked out again.
type ThumbCache = Mutex<Option<HashMap<String, (Arc<Skin>, Arc<Thumb>)>>>;
static CACHE: ThumbCache = Mutex::new(None);

/// The miniature of `row`, from the cache when its skin has not changed.
pub fn thumb(row: &SkinRow) -> Arc<Thumb> {
    let Some(skin) = row.skin.as_ref().filter(|_| row.error.is_none()) else {
        return Arc::new(Thumb::fallback(row));
    };
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some((cached, t)) = cache.get(&row.id)
        && Arc::ptr_eq(cached, skin)
    {
        return t.clone();
    }
    let urls = match &row.folder {
        Some(f)
            if skin.dialogue.box_.frame.is_some() || skin.dialogue.nameplate.frame.is_some() =>
        {
            crate::assets::skin_urls(&nanatype_theme::DirFiles::new(f))
        }
        _ => HashMap::new(),
    };
    let t = Arc::new(Thumb::of(skin, &urls));
    cache.insert(row.id.clone(), (skin.clone(), t.clone()));
    t
}

/// An absolutely placed node at `r`.
fn placed(r: Rect) -> Stack {
    Stack::column(0.0)
        .width(LengthSpec::Px(r[2]))
        .height(LengthSpec::Px(r[3]))
        .with_layout(move |l| {
            l.position = PositionSpec::Absolute;
            l.offset_left = Some(LengthSpec::Px(r[0]));
            l.offset_top = Some(LengthSpec::Px(r[1]));
        })
}

/// A rounded bar standing in for a run of text.
fn bar(w: LengthSpec, h: f32, color: [f32; 4], stroke: Option<([f32; 4], f32)>) -> AnyView {
    widget(
        Stack::row(0.0)
            .width(w)
            .height(LengthSpec::Px(h))
            .with_layout(move |l| {
                l.background = Some(color);
                l.border_radius = Some(h / 2.0);
                l.flex_shrink = Some(0.0);
                if let Some((color, width)) = stroke {
                    l.border_width = Some(width);
                    l.border_color = Some(color);
                    l.border_style = Some(nana_ui_core::BorderStyle::Solid);
                }
            }),
    )
    .into_any()
}

/// The miniature, drawn at its scale.
pub fn miniature(t: &Thumb) -> AnyView {
    let [w, h] = t.size;
    let scale = t.scale;
    let [pl, pt, pr, pb] = t.padding;
    let align = match t.align {
        TextAlign::Start => nana_ui_core::AlignSpec::Start,
        TextAlign::Center => nana_ui_core::AlignSpec::Center,
        TextAlign::End => nana_ui_core::AlignSpec::End,
    };
    let widths = [78.0, 52.0];
    let lines: Vec<AnyView> = (0..t.lines)
        .map(|i| {
            bar(
                LengthSpec::Percent(widths[i]),
                t.bar,
                t.text_color,
                t.stroke,
            )
        })
        .collect();
    let surface = t.box_surface.clone();
    let gap = (t.pitch - t.bar).max(2.0);
    let top = pt + (t.pitch - t.bar) / 2.0;
    let dot = (t.bar * 0.9).max(6.0);
    let the_box = widget(placed(t.box_rect).with_layout(move |l| surface.apply(l))).children((
        widget(
            Stack::column(gap)
                .width(LengthSpec::Fill)
                .align(align)
                .with_layout(move |l| {
                    l.padding_left = Some(LengthSpec::Px(pl));
                    l.padding_top = Some(LengthSpec::Px(top));
                    l.padding_right = Some(LengthSpec::Px(pr));
                }),
        )
        .children(lines),
        // The accent, where the continue mark sits.
        widget(
            Stack::row(0.0)
                .width(LengthSpec::Px(dot))
                .height(LengthSpec::Px(dot))
                .with_layout({
                    let accent = t.accent;
                    move |l| {
                        l.position = PositionSpec::Absolute;
                        l.offset_right = Some(LengthSpec::Px(pr.max(dot)));
                        l.offset_bottom = Some(LengthSpec::Px(pb.max(dot) * 0.8));
                        l.background = Some(accent);
                        l.border_radius = Some(dot / 2.0);
                    }
                }),
        ),
    ));
    let plate = t.plate.as_ref().map(|p| {
        let surface = p.surface.clone();
        let rect = p.rect;
        widget(
            placed(rect)
                .align(nana_ui_core::AlignSpec::Center)
                .justify(nana_ui_core::JustifySpec::Center)
                .with_layout(move |l| {
                    surface.apply(l);
                    // `apply` may have rotated it in place; keep the offsets.
                    l.position = PositionSpec::Absolute;
                }),
        )
        .children(bar(LengthSpec::Px(p.bar[0]), p.bar[1], p.text_color, None))
    });
    widget(
        Stack::row(0.0)
            .width(LengthSpec::Px(w * scale))
            .height(LengthSpec::Px(h * scale))
            .with_layout(|l| {
                l.position = PositionSpec::Relative;
                l.flex_shrink = Some(0.0);
                l.pointer_events = Some(PointerEventsSpec::None);
            }),
    )
    .children(
        widget(
            Stack::column(0.0)
                .width(LengthSpec::Px(w))
                .height(LengthSpec::Px(h))
                .with_layout(move |l| {
                    l.position = PositionSpec::Absolute;
                    l.offset_left = Some(LengthSpec::Px(0.0));
                    l.offset_top = Some(LengthSpec::Px(0.0));
                    l.overflow_x = OverflowSpec::Visible;
                    l.overflow_y = OverflowSpec::Visible;
                    l.transform = Some(affine(0.0, 0.0, 0.0, scale));
                    l.transform_origin = Some(TransformOrigin {
                        x: LengthSpec::Px(0.0),
                        y: LengthSpec::Px(0.0),
                    });
                }),
        )
        .children((the_box, plate)),
    )
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_miniature_fits_its_slot() {
        let registry = nanatype_theme::SkinRegistry::new(None, env!("CARGO_PKG_VERSION"));
        let rows = crate::ui::skin_rows(&registry, nanatype_i18n::Locale::EnUs);
        assert!(rows.len() >= 18);
        for row in &rows {
            let t = thumb(row);
            assert!(row.skin.is_some(), "{} did not load", row.id);
            let [w, h] = t.size.map(|v| v * t.scale);
            assert!(
                w <= FIT[0] + 0.5 && h <= FIT[1] + 0.5,
                "{}: {w}×{h}",
                row.id
            );
            assert!(w >= 60.0, "{} is a sliver: {w}×{h}", row.id);
            assert!(Arc::ptr_eq(&t, &thumb(row)), "{} was not cached", row.id);
        }
    }

    #[test]
    fn a_skin_that_did_not_load_gets_a_plain_box() {
        let registry = nanatype_theme::SkinRegistry::new(None, env!("CARGO_PKG_VERSION"));
        let mut row = crate::ui::skin_rows(&registry, nanatype_i18n::Locale::EnUs).remove(0);
        row.skin = None;
        row.error = Some("broken".into());
        let t = thumb(&row);
        assert!(t.plate.is_none());
        assert_eq!(t.box_surface.fill, Some(row.box_color));
    }
}
