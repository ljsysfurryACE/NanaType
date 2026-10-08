//! The dialogue box.
//!
//! One builder draws the typewriter, the input editor's box and the history
//! bubbles, so they cannot drift apart. The text itself is a single NanaUI
//! rich text node (a `RichTextView`, or the `RichTextEditor` in the input
//! window) built from the document by [`super::rich`]; the typewriter reveal
//! and the per-character effects are glyph presentation evaluated on the GPU,
//! so nothing reshapes while a line plays.

use std::f32::consts::TAU;
use std::sync::Arc;

use nana_ui::runtime::view::{AnyView, IntoView, dynamic, widget};
use nana_ui::runtime::{NodeStyle, RichTextView, Stack, Text};
use nana_ui_core::{
    AlignSpec, BackgroundImage, BackgroundImageFit, BorderImageSlice, BorderImageSpec,
    BoxShadowSpec, LayoutStyle, LengthSpec, OverflowSpec, PaintTransform, PointerEventsSpec,
    PositionSpec, TextShadowSpec, TextStrokeSpec,
};
use nanatype_core::Reveal;
use nanatype_model::{Document, Entrance, Inline, Rgba8};
use nanatype_theme::{
    EffectDefaults, NineSlice, Side, Skin, TextShadowSpec as SkinShadow,
    TextStrokeSpec as SkinStroke,
};

use super::rich::{Env, Look, RevealPlan, effect_table, to_rich};
use super::{Ui, c};

/// Base text look a document is drawn with (skin dialogue text or history text).
#[derive(Clone, PartialEq)]
pub struct Base {
    pub color: Rgba8,
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
    pub family: Option<String>,
    pub name_family: Option<String>,
    pub accent_family: Option<String>,
    pub letter_spacing: f32,
    pub shadow: Option<SkinShadow>,
    pub stroke: Option<SkinStroke>,
    pub effects: EffectDefaults,
    pub entrance: Entrance,
    pub reduced_motion: bool,
}

impl Base {
    fn from_text(
        skin: &Skin,
        t: &nanatype_theme::TextSpec,
        entrance: Entrance,
        reduced: bool,
    ) -> Self {
        Self {
            color: t.color,
            size: t.size,
            line_height: t.line_height,
            weight: skin.fonts.body.weight,
            family: skin.fonts.body.families.first().cloned(),
            name_family: skin.fonts.name.families.first().cloned(),
            accent_family: skin.fonts.accent.families.first().cloned(),
            letter_spacing: t.letter_spacing,
            shadow: t.shadow.clone(),
            stroke: t.stroke.clone(),
            effects: skin.effects.clone(),
            entrance,
            reduced_motion: reduced,
        }
    }

    pub fn dialogue(skin: &Skin, entrance: Entrance, reduced: bool) -> Self {
        Self::from_text(skin, &skin.dialogue.text, entrance, reduced)
    }

    /// The input editor's text: the dialogue text, nothing animating in.
    pub fn editor(skin: &Skin) -> Self {
        let entrance = Entrance {
            kind: nanatype_model::EntranceKind::None,
            ms: 0,
        };
        Self::dialogue(skin, entrance, false)
    }

    pub fn history(skin: &Skin, entrance: Entrance, reduced: bool) -> Self {
        Self::from_text(skin, &skin.history.text, entrance, reduced)
    }

    pub fn look(&self) -> Look {
        Look {
            size: self.size,
            color: self.color,
            weight: self.weight,
            line_height: self.line_height,
            body: self.family.clone(),
            name: self.name_family.clone(),
            accent: self.accent_family.clone(),
        }
    }

    /// The node style every rich text of this look shares: size, family,
    /// colour, line height and the skin's outline and shadow.
    pub fn node_style(&self) -> NodeStyle {
        let mut st = NodeStyle::default();
        let l = Arc::make_mut(&mut st.layout);
        l.font_size = Some(self.size);
        l.font_weight = Some(self.weight);
        l.font_family = self.family.clone();
        l.letter_spacing = Some(self.letter_spacing);
        l.color = Some(c(self.color));
        l.line_height = Some(nana_ui_core::LineHeightSpec::Absolute(
            self.size * self.line_height,
        ));
        if let Some(s) = &self.shadow {
            l.paint.text_shadows = vec![TextShadowSpec {
                offset_x: s.dx,
                offset_y: s.dy,
                blur_radius: s.blur,
                color: c(s.color),
                paint_color: None,
            }];
        }
        if let Some(s) = &self.stroke {
            l.paint.text_stroke = Some(TextStrokeSpec {
                width: s.width,
                color: Some(c(s.color)),
                paint_color: None,
            });
            l.paint.paint_order_stroke_first = true;
        }
        st
    }
}

/// How a document's units appear over time.
#[derive(Clone)]
pub enum Timing {
    /// Everything shown, nothing animates on entry.
    Static,
    /// Per-unit reveal times from a [`Reveal`].
    Reveal(Reveal),
}

/// `doc` as one rich text node in `window`, with its reveal and effects.
pub fn document(ui: Ui, doc: &Document, base: &Base, timing: Timing) -> AnyView {
    let urls = ui.asset_urls.get_untracked();
    let look = base.look();
    let env = Env {
        look: &look,
        urls: &urls,
        editor: false,
        locale: ui.locale.get_untracked(),
    };
    let rich = to_rich(doc, &env);
    let reveal = match &timing {
        Timing::Static => None,
        Timing::Reveal(r) => Some(RevealPlan::new(&rich, r, base.entrance)),
    };
    let mut view = RichTextView::new(rich)
        .decorative()
        .style(base.node_style())
        .width(LengthSpec::Fill);
    if reveal.is_some() || has_effects(doc) {
        // The schedule's start is taken by the framework as the frame the
        // node first shows, which is "now" for the offsets computed here.
        let schedule = reveal.map(|plan| {
            plan.schedule(
                std::time::Duration::ZERO,
                crate::clock::now_ms(),
                base.reduced_motion,
            )
        });
        view = view.presentation(effect_table(&base.effects, base.reduced_motion), schedule);
    }
    widget(view).into_any()
}

fn nine_slice(frame: &NineSlice, url: String) -> BorderImageSpec {
    let mut spec = BorderImageSpec::from_source(BackgroundImage::url(url));
    spec.slice = frame.slice.map(|v| BorderImageSlice::Number(v as f32));
    spec.fill = true;
    spec
}

/// The box chrome (fill, border, glow, frame image) for the skin.
fn box_layout(skin: &Skin, l: &mut LayoutStyle, frame_url: Option<String>) {
    let b = &skin.dialogue.box_;
    if let (Some(frame), Some(url)) = (&b.frame, frame_url) {
        l.paint.border_image = Some(nine_slice(frame, url));
        l.border_top_width = Some(frame.slice[0] as f32 * frame.scale);
        l.border_right_width = Some(frame.slice[1] as f32 * frame.scale);
        l.border_bottom_width = Some(frame.slice[2] as f32 * frame.scale);
        l.border_left_width = Some(frame.slice[3] as f32 * frame.scale);
        return;
    }
    l.background = Some(c(b.color));
    l.border_radius = Some(b.radius);
    if b.border_width > 0.0 {
        l.border_width = Some(b.border_width);
        l.border_color = Some(c(b.border_color));
        l.border_style = Some(nana_ui_core::BorderStyle::Solid);
    }
    let mut shadows = Vec::new();
    if let Some(inner) = b.inner_border_color {
        // A second border drawn just inside the first.
        shadows.push(BoxShadowSpec {
            paint_color: None,
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            spread_radius: b.inner_border_width.max(1.0),
            color: c(inner),
            inset: true,
        });
    }
    if let Some(glow) = b.glow {
        shadows.push(BoxShadowSpec {
            paint_color: None,
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: b.glow_blur,
            spread_radius: 0.0,
            color: c(glow),
            inset: false,
        });
    }
    if let Some(s) = &b.shadow {
        shadows.push(BoxShadowSpec {
            paint_color: None,
            offset_x: s.dx,
            offset_y: s.dy,
            blur_radius: s.blur,
            spread_radius: 0.0,
            color: c(s.color),
            inset: false,
        });
    }
    l.paint.box_shadows = shadows;
}
/// What a dialogue box shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The typewriter: the display view with its reveal.
    Display,
    /// The input window: the editor sits where the text goes.
    Draft,
}

/// A complete dialogue box filling its parent (region + overscan). In the
/// input window `editor` takes the text's place; it is built once so it
/// keeps its selection and undo history.
pub fn dialogue_box(ui: Ui, source: Source, editor: Option<AnyView>) -> AnyView {
    if let Some(editor) = editor {
        return box_view(ui, source, editor).into_any();
    }
    let key = move || {
        let skin = ui.skin.get();
        let settings = ui.settings.get();
        (
            Arc::as_ptr(&skin) as usize,
            Arc::as_ptr(&settings) as usize,
            ui.locale.get(),
            ui.portraits.with(|p| Arc::as_ptr(p) as usize),
        )
    };
    dynamic(key, move |_| box_view(ui, source, content(ui).into_any())).into_any()
}

fn box_view(ui: Ui, source: Source, content: AnyView) -> impl IntoView {
    let skin = ui.skin.get_untracked();
    let d = &skin.dialogue;
    let overscan = d.overscan;
    let frame_url = ui.asset_urls.with_untracked(|urls| {
        d.box_
            .frame
            .as_ref()
            .and_then(|f| urls.get(&format!("skin:{}", f.image)).cloned())
    });
    let width_fraction = d.box_.width_fraction;
    let padding = d.padding;
    let skin_box = skin.clone();
    let plate = nameplate(ui, source);
    let portrait = portrait(ui);
    let cont = continue_indicator(ui, source);
    widget(Stack::fill_column(0.0).with_layout(move |l| {
        l.padding_left = Some(LengthSpec::Px(overscan));
        l.padding_right = Some(LengthSpec::Px(overscan));
        l.padding_top = Some(LengthSpec::Px(overscan));
        l.padding_bottom = Some(LengthSpec::Px(overscan));
        l.align_items = AlignSpec::Center;
    }))
    .bind(move |s: &mut Stack| {
        let editing = source == Source::Display
            && ui
                .settings
                .with(|x| x.overlays.display.lock == nanatype_settings::Lock::EditLayout);
        let accent = c(ui.skin.with(|k| k.chrome.accent));
        let mut taken = std::mem::replace(s, Stack::row(0.0));
        taken = taken.with_layout(|l| edit_outline(l, editing, accent));
        *s = taken;
    })
    .children(
        widget(
            Stack::fill_column(0.0)
                .width(LengthSpec::Percent(width_fraction * 100.0))
                .with_layout(move |l| {
                    l.position = PositionSpec::Relative;
                    box_layout(&skin_box, l, frame_url);
                    l.padding_left = Some(LengthSpec::Px(padding[0]));
                    l.padding_top = Some(LengthSpec::Px(padding[1]));
                    l.padding_right = Some(LengthSpec::Px(padding[2]));
                    l.padding_bottom = Some(LengthSpec::Px(padding[3]));
                    l.overflow_x = OverflowSpec::Visible;
                    l.overflow_y = OverflowSpec::Visible;
                }),
        )
        .bind(move |s: &mut Stack| {
            // Whole-box fade and screen shake.
            let view = ui.display.get();
            let now = ui.now.get();
            let (opacity, dx, dy) = if source == Source::Display {
                let fade = view
                    .fade
                    .map(|(at, ms)| {
                        1.0 - (now.saturating_sub(at) as f32 / ms.max(1) as f32).clamp(0.0, 1.0)
                    })
                    .unwrap_or(1.0);
                let opacity = if view.empty { 0.0 } else { fade };
                let (dx, dy) = match view.shake {
                    Some(sh) if now < sh.at_ms + u64::from(sh.ms) && !ui.reduced_motion.get() => {
                        let left = 1.0 - (now - sh.at_ms) as f32 / sh.ms.max(1) as f32;
                        let step = now / 16;
                        (
                            noise(1, step) * sh.amp * left,
                            noise(2, step) * sh.amp * left,
                        )
                    }
                    _ => (0.0, 0.0),
                };
                (opacity, dx, dy)
            } else {
                (1.0, 0.0, 0.0)
            };
            let mut taken = std::mem::replace(s, Stack::row(0.0));
            taken = taken.with_layout(|l| {
                l.opacity = Some(opacity);
                l.transform = (dx != 0.0 || dy != 0.0).then_some(PaintTransform {
                    a: 1.0,
                    b: 0.0,
                    c: 0.0,
                    d: 1.0,
                    e: dx,
                    f: dy,
                });
            });
            *s = taken;
        })
        .children((
            portrait,
            plate,
            widget(Stack::fill_column(0.0).with_layout(|l| {
                l.overflow_y = OverflowSpec::Hidden;
            }))
            .children(content),
            cont,
        )),
    )
}

/// A dashed frame and a faint fill while a window is in layout mode, so it
/// can be found and dragged even when empty.
pub fn edit_outline(l: &mut LayoutStyle, editing: bool, accent: [f32; 4]) {
    if editing {
        l.border_width = Some(2.0);
        l.border_color = Some(accent);
        l.border_style = Some(nana_ui_core::BorderStyle::Dashed);
        l.background = Some([accent[0], accent[1], accent[2], 0.08]);
    } else {
        l.border_width = None;
        l.border_style = None;
        l.background = None;
    }
}

/// The typewriter's text, rebuilt when the display view changes.
fn content(ui: Ui) -> impl IntoView {
    dynamic(
        move || ui.display.get().revision,
        move |_| {
            let skin = ui.skin.get_untracked();
            let reduced = ui.reduced_motion.get_untracked();
            let view = ui.display.get_untracked();
            if view.empty {
                return widget(Stack::column(0.0)).into_any();
            }
            let base = Base::dialogue(&skin, view.entrance, reduced);
            document(ui, &view.doc, &base, Timing::Reveal(view.reveal.clone()))
        },
    )
}

/// Pseudo-random in -1..1 from an integer, stable across frames.
fn noise(i: u32, step: u64) -> f32 {
    let mut x = (u64::from(i) << 32 ^ step).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x as u32) as f32 / u32::MAX as f32 * 2.0 - 1.0
}

fn nameplate(ui: Ui, source: Source) -> impl IntoView {
    let skin = ui.skin.get_untracked();
    let p = skin.dialogue.nameplate.clone();
    let (ox, oy) = content_origin(&skin);
    let body_family = skin.fonts.name.families.first().cloned();
    let name = move || match source {
        Source::Display => ui.display.with(|v| v.speaker.name.clone()),
        Source::Draft => ui.settings.with(|s| s.general.speaker_name.clone()),
    };
    let mut st = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.font_size = Some(p.size);
        l.font_weight = Some(skin.fonts.name.weight.max(600));
        l.font_family = body_family;
        l.color = Some(c(p.text_color));
        if let Some(stroke) = &p.stroke {
            l.paint.text_shadow = Some(TextShadowSpec {
                offset_x: 0.0,
                offset_y: 0.0,
                blur_radius: stroke.width * 1.5,
                color: c(stroke.color),
                paint_color: None,
            });
        }
    }
    let visible = p.visible;
    widget(
        Stack::row(0.0)
            .padding_xy(p.padding[0], p.padding[1])
            .with_layout(move |l| {
                l.position = PositionSpec::Absolute;
                l.offset_left = Some(LengthSpec::Px(p.offset[0] - ox));
                l.offset_top = Some(LengthSpec::Px(p.offset[1] - oy));
                l.background = Some(c(p.color));
                l.border_radius = Some(p.radius);
                l.z_index = Some(2);
            }),
    )
    .visible(move || {
        visible
            && !name().trim().is_empty()
            && !(source == Source::Display && ui.display.with(|v| v.empty))
    })
    .children(widget(Text::new("").style(st)).bind(move |t: &mut Text| {
        let n = match source {
            Source::Display => ui.display.with(|v| v.speaker.name.clone()),
            Source::Draft => ui.settings.with(|s| s.general.speaker_name.clone()),
        };
        if t.value != n {
            t.value = n;
        }
    }))
}

fn portrait(ui: Ui) -> impl IntoView {
    let skin = ui.skin.get_untracked();
    let p = skin.dialogue.portrait.clone();
    let [w, h] = p.size;
    let pad = skin.dialogue.padding;
    widget(
        Stack::row(0.0)
            .width(LengthSpec::Px(w))
            .height(LengthSpec::Px(h))
            .with_layout(move |l| {
                l.position = PositionSpec::Absolute;
                l.offset_bottom = Some(LengthSpec::Px(p.offset[1] - pad[3]));
                match p.side {
                    Side::Left => {
                        l.offset_left = Some(LengthSpec::Px(-w * 0.5 + p.offset[0] - pad[0]))
                    }
                    Side::Right => {
                        l.offset_right = Some(LengthSpec::Px(-w * 0.5 - p.offset[0] - pad[2]))
                    }
                }
                l.border_radius = Some(p.radius);
                l.pointer_events = Some(PointerEventsSpec::None);
            }),
    )
    .visible(move || ui.skin.with(|s| s.dialogue.portrait.visible) && !ui.display.with(|v| v.empty))
    .bind(move |s: &mut Stack| {
        let variant = ui.display.with(|v| v.portrait.clone());
        let url = ui.portraits.with(|map| {
            variant
                .as_ref()
                .and_then(|v| map.get(v))
                .or_else(|| map.get("default"))
                .cloned()
        });
        let mut taken = std::mem::replace(s, Stack::row(0.0));
        taken = taken.with_layout(|l| {
            l.paint.background_image =
                url.map(|u| BackgroundImage::url_with_fit(u, BackgroundImageFit::Contain));
        });
        *s = taken;
    })
}

fn continue_indicator(ui: Ui, source: Source) -> impl IntoView {
    let skin = ui.skin.get_untracked();
    let cs = skin.dialogue.continue_.clone();
    let url = cs.image.as_ref().map(|i| format!("skin:{i}"));
    let image = url.and_then(|u| ui.asset_urls.with_untracked(|m| m.get(&u).cloned()));
    let mut st = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut st.layout);
        l.font_size = Some(cs.size);
        l.color = Some(c(cs.color));
    }
    let size = cs.size;
    let bob = cs.bob_px;
    let child: AnyView = match image {
        Some(u) => widget(
            Stack::row(0.0)
                .width(LengthSpec::Px(size * 1.4))
                .height(LengthSpec::Px(size * 1.4))
                .with_layout(move |l| {
                    l.paint.background_image = Some(BackgroundImage::url_with_fit(
                        u,
                        BackgroundImageFit::Contain,
                    ));
                }),
        )
        .into_any(),
        None => widget(Text::new(cs.glyph.clone()).decorative().style(st)).into_any(),
    };
    let pad = skin.dialogue.padding;
    widget(Stack::row(0.0).with_layout(move |l| {
        l.position = PositionSpec::Absolute;
        l.offset_right = Some(LengthSpec::Px(14.0 - pad[2]));
        l.offset_bottom = Some(LengthSpec::Px(8.0 - pad[3]));
    }))
    .visible(move || {
        source == Source::Display && ui.display.with(|v| v.continue_indicator && !v.empty)
    })
    .bind(move |s: &mut Stack| {
        let now = ui.now.get();
        let dy = if ui.reduced_motion.get() {
            0.0
        } else {
            ((now as f32 / 1000.0) * TAU * 1.2).sin().abs() * -bob
        };
        let mut taken = std::mem::replace(s, Stack::row(0.0));
        taken = taken.with_layout(|l| {
            l.transform = Some(PaintTransform {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 0.0,
                f: dy,
            });
        });
        *s = taken;
    })
    .children(child)
}

/// Whether the display needs frames for the box itself (fade, shake, the
/// continue indicator). Text reveal and effects request their own frames.
pub fn display_animating(ui: Ui, now: u64) -> bool {
    let view = ui.display.get_untracked();
    if view.empty {
        return false;
    }
    if let Some((at, ms)) = view.fade
        && now <= at + u64::from(ms) + 50
    {
        return true;
    }
    if let Some(sh) = view.shake
        && now <= sh.at_ms + u64::from(sh.ms) + 50
    {
        return true;
    }
    view.continue_indicator && !ui.reduced_motion.get_untracked()
}

pub fn has_effects(doc: &Document) -> bool {
    doc.paragraphs.iter().any(|p| {
        p.inlines.iter().any(|i| match i {
            Inline::Text(r) => !r.effects.continuous.is_empty(),
            Inline::Ruby(r) => !r.base.effects.continuous.is_empty(),
            Inline::Atom(a) => !a.effects.continuous.is_empty(),
            Inline::Marker(_) => false,
        })
    })
}

/// Absolute children are placed from the box's content edge; skin offsets
/// are from its border edge.
fn content_origin(skin: &Skin) -> (f32, f32) {
    let pad = skin.dialogue.padding;
    (pad[0], pad[1])
}
