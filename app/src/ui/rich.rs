//! NanaType documents ⇄ NanaUI rich text.
//!
//! The dialogue box, the history bubbles and the editor are all NanaUI rich
//! text nodes. A [`Document`] becomes one `RichText`: runs become spans,
//! ruby groups become annotations, stickers become inline image objects,
//! mentions of chat viewers become styled labels with their avatar, and (in
//! the editor only) markers become labelled chips in the chrome's colours. Effects and pacing
//! have no rich-text field of their own, so each distinct combination is
//! interned in a table and the span carries its index as `effect`; the same
//! table yields the glyph effects the display plays. Converting back
//! ([`from_rich`]) restores the exact document the editor holds.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use nana_ui_core::{
    GlyphEffect, GlyphIntro, PaintColor, RevealSchedule, RichLabelStyle, RichObject,
    RichObjectContent, RichSpanStyle, RichText, RichTextShadow, RichTextStroke, TextDecorationLine,
    TextStrokePlacement,
};
use nanatype_core::Reveal;
use nanatype_i18n::Locale;
use nanatype_model::{
    ContinuousKind, Document, Effects, Entrance, EntranceKind, FontSlot, Inline, Marker, Pace,
    Paragraph, Rgba8, RubyGroup, Shadow, Size, Stroke, Style, TextRun,
};
use nanatype_theme::EffectDefaults;
use unicode_segmentation::UnicodeSegmentation;

/// How a document is drawn: the node's base text look.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub size: f32,
    pub color: Rgba8,
    pub weight: u16,
    pub line_height: f32,
    pub body: Option<String>,
    pub name: Option<String>,
    pub accent: Option<String>,
    pub mention: MentionLook,
}

/// How a mentioned chat viewer is drawn: the skin's `[mention]`. It also
/// carries the editor's marker chips, the other inline labels a skin colours.
#[derive(Debug, Clone, PartialEq)]
pub struct MentionLook {
    pub fill: [f32; 4],
    pub text: [f32; 4],
    pub scale: f32,
    pub avatar: bool,
    pub markers: MarkerLook,
}

/// How the editor draws markers: chips in the skin's chrome colours (an
/// accent wash with readable ink), each kind told apart by a glyph and a
/// coloured stripe rather than by its fill alone.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerLook {
    pub fill: Rgba8,
    pub text: Rgba8,
}

impl MarkerLook {
    pub fn from_skin(skin: &nanatype_theme::Skin) -> Self {
        let c = &skin.chrome;
        let fill = mix(c.surface.with_alpha(255), c.accent.with_alpha(255), 0.16);
        Self {
            fill,
            text: crate::skin_theme::on(fill),
        }
    }

    /// A marker (or a missing picture) as a chip: the label after its kind's
    /// glyph and stripe.
    pub fn object(&self, id: u64, label: impl Into<Arc<str>>, kind: ChipKind) -> RichObject {
        RichObject::label(
            id,
            label,
            RichLabelStyle {
                fill: crate::skin_theme::rgba(self.fill),
                text: crate::skin_theme::rgba(self.text),
                scale: 0.62,
                weight: Some(600),
                radius_px: Some(4.0),
                icon: Some(kind.icon(self.text)),
                icon_round: false,
            },
        )
    }
}

fn mix(a: Rgba8, b: Rgba8, t: f32) -> Rgba8 {
    let m = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    Rgba8::rgba(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b), m(a.a, b.a))
}

/// What a marker chip stands for: its glyph and the hue of its stripe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChipKind {
    /// `[wait]`: a clock.
    Wait,
    /// `[pause]`: wait for continue, a play mark.
    Continue,
    /// `[page]`: a page break.
    Page,
    /// `[se]`: a note.
    Sound,
    /// `[expr]`, portraits and VTube Studio: a face.
    Expression,
    /// `[screenshake]`: a zigzag.
    Shake,
    /// A sticker or picture whose file is missing.
    Picture,
    /// `[at]`: a mention.
    Mention,
}

impl ChipKind {
    pub fn of(m: &Marker) -> Self {
        match m {
            Marker::Wait { .. } => Self::Wait,
            Marker::Pause { .. } => Self::Continue,
            Marker::Page => Self::Page,
            Marker::Sound { .. } => Self::Sound,
            Marker::Expression { .. } | Marker::Portrait { .. } | Marker::Vts(_) => {
                Self::Expression
            }
            Marker::ScreenShake { .. } => Self::Shake,
        }
    }

    /// The stripe's hue: pacing orange, sounds green, faces purple, shake
    /// pink, pictures slate, mentions blue.
    pub fn hue(self) -> Rgba8 {
        match self {
            Self::Wait | Self::Continue | Self::Page => Rgba8::rgb(237, 140, 51),
            Self::Sound => Rgba8::rgb(56, 168, 140),
            Self::Expression => Rgba8::rgb(140, 102, 217),
            Self::Shake => Rgba8::rgb(224, 97, 140),
            Self::Picture => Rgba8::rgb(115, 128, 148),
            Self::Mention => Rgba8::rgb(61, 133, 235),
        }
    }

    /// The glyph, as 24-unit SVG strokes (Tabler's grid).
    fn glyph(self) -> &'static str {
        match self {
            Self::Wait => r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>"#,
            Self::Continue => r#"<path d="M8 5.5v13l10-6.5z" fill="INK"/>"#,
            Self::Page => r#"<path d="M5 4.5h14M5 19.5h14M12 8v8M9 13l3 3 3-3"/>"#,
            Self::Sound => {
                r#"<circle cx="7.5" cy="17" r="2.5"/><circle cx="17" cy="15" r="2.5"/><path d="M10 17V6l9.5-2v11"/>"#
            }
            Self::Expression => {
                r#"<circle cx="12" cy="12" r="8.5"/><path d="M9 10h.01M15 10h.01M9 14.5a4 4 0 0 0 6 0"/>"#
            }
            Self::Shake => r#"<path d="M3 12l3-5 4 10 4-10 4 10 3-5"/>"#,
            Self::Picture => {
                r#"<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M4 16l5-5 5 5M14 14l2-2 4 4"/>"#
            }
            Self::Mention => {
                r#"<circle cx="12" cy="12" r="3.5"/><path d="M15.5 12v1.5a2.5 2.5 0 0 0 5 0V12a8.5 8.5 0 1 0-3.5 6.9"/>"#
            }
        }
    }

    /// The chip's picture: the hue as a stripe down its left edge and the
    /// glyph in `ink` beside it, as an SVG data URL.
    pub fn icon(self, ink: Rgba8) -> Arc<str> {
        static CACHE: LazyLock<Mutex<HashMap<(ChipKind, Rgba8), Arc<str>>>> =
            LazyLock::new(Default::default);
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(cache.entry((self, ink)).or_insert_with(|| {
            let rgb = |c: Rgba8| format!("rgb({},{},{})", c.r, c.g, c.b);
            let (ink, hue) = (rgb(ink), rgb(self.hue()));
            let glyph = self.glyph().replace("INK", &ink);
            Arc::from(format!(
                "data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"48\" height=\"48\" viewBox=\"0 0 24 24\">\
                 <rect x=\"0\" y=\"0\" width=\"3\" height=\"24\" rx=\"1\" fill=\"{hue}\"/>\
                 <g transform=\"translate(6.5 3) scale(0.75)\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"2.4\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{glyph}</g></svg>"
            ))
        }))
    }
}

impl Default for MentionLook {
    fn default() -> Self {
        Self::from_skin(&nanatype_theme::Skin::default())
    }
}

impl MentionLook {
    pub fn from_skin(skin: &nanatype_theme::Skin) -> Self {
        let m = &skin.mention;
        Self {
            fill: crate::skin_theme::rgba(skin.color(m.fill)),
            text: crate::skin_theme::rgba(skin.color(m.text_color)),
            scale: if m.scale.is_finite() {
                m.scale.clamp(0.3, 2.0)
            } else {
                0.82
            },
            avatar: m.avatar,
            markers: MarkerLook::from_skin(skin),
        }
    }

    /// The label for `mention`, with its avatar when one has a URL.
    pub fn object(
        &self,
        id: u64,
        mention: &nanatype_model::Mention,
        urls: &HashMap<String, String>,
    ) -> RichObject {
        let icon = self
            .avatar
            .then_some(mention.user.avatar.as_ref())
            .flatten()
            .and_then(|a| urls.get(&a.to_string()))
            .map(|u| Arc::<str>::from(u.as_str()));
        RichObject::label(
            id,
            mention.alt(),
            RichLabelStyle {
                fill: self.fill,
                text: self.text,
                scale: self.scale,
                weight: Some(700),
                icon,
                ..RichLabelStyle::default()
            },
        )
    }
}

// ---- interned attribute combinations -----------------------------------

static ATTRS: LazyLock<Mutex<Vec<(Effects, Pace)>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// The span `effect` index for these effects and pacing (`None` = plain).
pub fn attr_index(effects: &Effects, pace: Pace) -> Option<u16> {
    if effects.is_none() && pace.is_inherit() {
        return None;
    }
    let mut table = ATTRS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = table.iter().position(|(e, p)| e == effects && *p == pace) {
        return Some(i as u16);
    }
    table.push((effects.clone(), pace));
    Some((table.len() - 1) as u16)
}

/// Like [`attr_index`] but always an index, so a toolbar can switch a span
/// back to no effect (span fields can only be overlaid, not removed).
pub fn attr_index_any(effects: &Effects, pace: Pace) -> u16 {
    if let Some(i) = attr_index(effects, pace) {
        return i;
    }
    let mut table = ATTRS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = table
        .iter()
        .position(|(e, p)| e.is_none() && p.is_inherit())
    {
        return i as u16;
    }
    table.push((Effects::default(), Pace::Inherit));
    (table.len() - 1) as u16
}

pub fn attr_of(index: u16) -> (Effects, Pace) {
    ATTRS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(usize::from(index))
        .cloned()
        .unwrap_or((Effects::default(), Pace::Inherit))
}

fn glyph_effect(e: &nanatype_model::Continuous, d: &EffectDefaults) -> GlyphEffect {
    match e.kind {
        ContinuousKind::Shake => {
            GlyphEffect::shake(d.shake_px * e.amp).frequency(d.shake_hz * e.speed)
        }
        ContinuousKind::Wave => {
            GlyphEffect::wave(d.wave_px * e.amp).frequency(e.speed / d.wave_period_s.max(0.05))
        }
        ContinuousKind::Jump => {
            GlyphEffect::jump(d.jump_px * e.amp).frequency(e.speed / d.jump_period_s.max(0.05))
        }
        ContinuousKind::Rainbow => {
            GlyphEffect::rainbow().frequency(e.speed / d.rainbow_period_s.max(0.05))
        }
    }
}

/// Glyph effects indexed like the attribute table. One effect plays per span:
/// the first of its continuous effects.
pub fn effect_table(defaults: &EffectDefaults, reduced: bool) -> Arc<[GlyphEffect]> {
    let table = ATTRS.lock().unwrap_or_else(|e| e.into_inner());
    table
        .iter()
        .map(|(effects, _)| match effects.continuous.first() {
            Some(e) if !reduced => glyph_effect(e, defaults),
            _ => GlyphEffect::shake(0.0),
        })
        .collect()
}

// ---- inline objects ------------------------------------------------------

struct Objects {
    next: u64,
    by_id: HashMap<u64, Inline>,
}

static OBJECTS: LazyLock<Mutex<Objects>> = LazyLock::new(|| {
    Mutex::new(Objects {
        next: 1,
        by_id: HashMap::new(),
    })
});

/// An object id for an atom, mention or marker; equal inlines share one id.
pub fn object_id(inline: &Inline) -> u64 {
    let mut o = OBJECTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((id, _)) = o.by_id.iter().find(|(_, v)| *v == inline) {
        return *id;
    }
    let id = o.next;
    o.next += 1;
    o.by_id.insert(id, inline.clone());
    id
}

pub fn object_of(id: u64) -> Option<Inline> {
    OBJECTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .by_id
        .get(&id)
        .cloned()
}

/// Width over height of an image file, cached.
fn aspect(url: &str) -> f32 {
    static CACHE: LazyLock<Mutex<HashMap<String, f32>>> = LazyLock::new(Default::default);
    if let Some(a) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).get(url) {
        return *a;
    }
    let path = url.strip_prefix("file:///").map(|p| {
        p.replace("%20", " ")
            .replace("%23", "#")
            .replace("%25", "%")
            .replace("%3F", "?")
    });
    let a = path
        .and_then(|p| imagesize::size(&p).ok())
        .map(|s| s.width as f32 / s.height.max(1) as f32)
        .filter(|a| a.is_finite() && *a > 0.0)
        .unwrap_or(1.0);
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(url.to_owned(), a);
    a
}

// ---- document → rich text -----------------------------------------------

fn color(c: Rgba8) -> PaintColor {
    PaintColor::srgb(crate::skin_theme::rgba(c))
}

fn span_style(style: &Style, effects: &Effects, pace: Pace, look: &Look) -> RichSpanStyle {
    let mut s = RichSpanStyle::new();
    if let Some(c) = style.color {
        s = s.color(color(c));
    }
    if let Some(size) = style.size {
        s = s.size(size.resolve(look.size));
    }
    match style.bold {
        Some(true) => s = s.weight(700.max(look.weight + 200).min(900)),
        Some(false) => s = s.weight(400.min(look.weight)),
        None => {}
    }
    if let Some(i) = style.italic {
        s = s.italic(i);
    }
    let family = match &style.font {
        Some(FontSlot::Name) => look.name.clone(),
        Some(FontSlot::Accent) => look.accent.clone(),
        Some(FontSlot::Family(f)) => Some(f.clone()),
        Some(FontSlot::Body) | None => None,
    };
    if let Some(f) = family {
        s = s.family(f);
    }
    if style.underline == Some(true) {
        s = s.underline();
    }
    if style.strike == Some(true) {
        s = s.line_through();
    }
    if let Some(st) = style.stroke {
        s = s.stroke(
            RichTextStroke::new(st.width_px, color(st.color)).placement(TextStrokePlacement::Under),
        );
    }
    if let Some(sh) = style.shadow {
        s = s.shadow(RichTextShadow::new(
            [sh.dx, sh.dy],
            sh.blur,
            color(sh.color),
        ));
    }
    if let Some(i) = attr_index(effects, pace) {
        s = s.effect(i);
    }
    s
}

/// What converting a document needs to know about its surroundings.
pub struct Env<'a> {
    pub look: &'a Look,
    pub urls: &'a HashMap<String, String>,
    /// The editor's text: markers as labelled tags (which take room, so
    /// the editor may break a line where the display does not).
    pub editor: bool,
    pub locale: Locale,
}

pub fn to_rich(doc: &Document, env: &Env<'_>) -> RichText {
    let mut b = RichText::builder();
    // Byte length so far, to style objects after building.
    let mut len = 0usize;
    let mut object_spans: Vec<(usize, RichSpanStyle)> = Vec::new();
    let replacement = nana_ui_core::OBJECT_REPLACEMENT.len_utf8();
    for (pi, para) in doc.paragraphs.iter().enumerate() {
        if pi > 0 {
            b.plain("\n");
            len += 1;
        }
        for inline in &para.inlines {
            match inline {
                Inline::Text(run) => {
                    let s = span_style(&run.style, &run.effects, run.pace, env.look);
                    if s.is_empty() {
                        b.plain(&run.text);
                    } else {
                        b.push(&run.text, s);
                    }
                    len += run.text.len();
                }
                Inline::Ruby(r) => {
                    let s = span_style(&r.base.style, &r.base.effects, r.base.pace, env.look);
                    b.styled_ruby(&r.base.text, r.annotation.clone(), s);
                    len += r.base.text.len();
                }
                Inline::Atom(a) => {
                    let id = object_id(inline);
                    let h = env.look.size * a.height_em;
                    let object = match env.urls.get(&a.asset.to_string()) {
                        Some(url) => {
                            RichObject::image(id, url.clone(), h * aspect(url), h).descent(h * 0.15)
                        }
                        // Missing picture: keep its place, labelled in the
                        // editor.
                        None if env.editor => {
                            let alt = if a.alt.is_empty() {
                                a.asset.to_string()
                            } else {
                                a.alt.clone()
                            };
                            env.look.mention.markers.object(id, alt, ChipKind::Picture)
                        }
                        None => RichObject::chip(id, a.alt.clone(), 5),
                    };
                    let s = span_style(&Style::default(), &a.effects, Pace::Inherit, env.look);
                    if !s.is_empty() {
                        object_spans.push((len, s));
                    }
                    b.object(object);
                    len += replacement;
                }
                Inline::Mention(m) => {
                    let id = object_id(inline);
                    let s = span_style(&Style::default(), &m.effects, Pace::Inherit, env.look);
                    if !s.is_empty() {
                        object_spans.push((len, s));
                    }
                    b.object(env.look.mention.object(id, m, env.urls));
                    len += replacement;
                }
                Inline::Marker(m) => {
                    if env.editor {
                        let id = object_id(inline);
                        let label = super::input::marker_label(m, env.locale);
                        b.object(env.look.mention.markers.object(id, label, ChipKind::of(m)));
                        len += replacement;
                    }
                }
            }
        }
    }
    let mut rich = b.build();
    for (start, style) in object_spans {
        rich.set_span(start..start + replacement, style);
    }
    rich
}

// ---- rich text → document -------------------------------------------------

fn style_back(s: Option<&RichSpanStyle>, look: &Look) -> (Style, Effects, Pace) {
    let Some(s) = s else {
        return (Style::default(), Effects::default(), Pace::Inherit);
    };
    let mut style = Style::default();
    if let Some(c) = s.paint.color {
        let [r, g, b, a] = c.to_srgb();
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        style.color = Some(Rgba8::rgba(q(r), q(g), q(b), q(a)));
    }
    if let Some(px) = s.shape.size_px {
        let em = (px / look.size.max(1.0) * 1000.0).round() / 1000.0;
        if (em - 1.0).abs() > 0.001 {
            style.size = Some(Size::Em(em));
        }
    }
    if let Some(w) = s.shape.weight {
        style.bold = Some(w >= 600);
    }
    style.italic = s.shape.italic;
    if let Some(f) = &s.shape.family {
        let f = f.to_string();
        style.font = Some(if Some(&f) == look.accent.as_ref() {
            FontSlot::Accent
        } else if Some(&f) == look.name.as_ref() {
            FontSlot::Name
        } else {
            FontSlot::Family(f)
        });
    }
    if let Some(TextDecorationLine {
        underline,
        line_through,
    }) = s.paint.decoration
    {
        style.underline = underline.then_some(true);
        style.strike = line_through.then_some(true);
    }
    let q = |c: PaintColor| {
        let [r, g, b, a] = c.to_srgb();
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgba8::rgba(q(r), q(g), q(b), q(a))
    };
    if let Some(st) = s.paint.stroke.as_ref().filter(|st| st.width_px > 0.0) {
        style.stroke = Some(Stroke {
            color: q(st.color),
            width_px: st.width_px,
        });
    }
    if let Some(sh) = s.paint.shadows.as_ref().and_then(|l| l.first()) {
        style.shadow = Some(Shadow {
            color: q(sh.color),
            dx: sh.offset[0],
            dy: sh.offset[1],
            blur: sh.blur_px,
        });
    }
    let (effects, pace) = s.effect.map(attr_of).unwrap_or_default();
    (style, effects, pace)
}

/// The document an editor holds. Exact inverse of [`to_rich`] for the editor.
pub fn from_rich(rich: &RichText, look: &Look) -> Document {
    let text = rich.text();
    let objects: HashMap<usize, &RichObject> =
        rich.objects().iter().map(|(o, ob)| (*o, ob)).collect();
    let rubies: Vec<(std::ops::Range<usize>, Arc<str>)> = rich.rubies().to_vec();
    let mut edges: Vec<usize> = vec![0, text.len()];
    for (range, _) in rich.spans().iter() {
        edges.push(range.start);
        edges.push(range.end);
    }
    for (offset, _) in rich.objects() {
        edges.push(*offset);
        edges.push(offset + nana_ui_core::OBJECT_REPLACEMENT.len_utf8());
    }
    for (range, _) in &rubies {
        edges.push(range.start);
        edges.push(range.end);
    }
    for (i, c) in text.char_indices() {
        if c == '\n' {
            edges.push(i);
            edges.push(i + 1);
        }
    }
    edges.sort_unstable();
    edges.dedup();

    let mut paragraphs = vec![Paragraph::default()];
    let mut pos = 0;
    while pos < text.len() {
        let current = paragraphs.last_mut().expect("one paragraph");
        if text[pos..].starts_with('\n') {
            paragraphs.push(Paragraph::default());
            pos += 1;
            continue;
        }
        if let Some(object) = objects.get(&pos) {
            let inline = object_of(object.id).or_else(|| match &object.content {
                RichObjectContent::Image { source } => Some(Inline::Atom(nanatype_model::Atom {
                    kind: nanatype_model::AtomKind::Image,
                    asset: nanatype_model::AssetRef::File(
                        source.trim_start_matches("file:///").replace("%20", " "),
                    ),
                    height_em: object.height_px / look.size.max(1.0),
                    alt: String::new(),
                    effects: Effects::default(),
                })),
                _ => None,
            });
            if let Some(mut inline) = inline {
                match &mut inline {
                    Inline::Atom(a) => a.effects = style_back(rich.style_at(pos), look).1,
                    Inline::Mention(m) => m.effects = style_back(rich.style_at(pos), look).1,
                    _ => {}
                }
                current.inlines.push(inline);
            }
            pos += nana_ui_core::OBJECT_REPLACEMENT.len_utf8();
            continue;
        }
        if let Some((range, annotation)) = rubies.iter().find(|(r, _)| r.start == pos) {
            let (style, effects, pace) = style_back(rich.style_at(pos), look);
            current.inlines.push(Inline::Ruby(RubyGroup {
                base: TextRun {
                    text: text[range.clone()].to_owned(),
                    style,
                    effects,
                    pace,
                },
                annotation: annotation.to_string(),
            }));
            pos = range.end;
            continue;
        }
        let end = edges
            .iter()
            .copied()
            .find(|e| *e > pos)
            .unwrap_or(text.len());
        let (style, effects, pace) = style_back(rich.style_at(pos), look);
        current.inlines.push(Inline::Text(TextRun {
            text: text[pos..end].to_owned(),
            style,
            effects,
            pace,
        }));
        pos = end;
    }
    Document { paragraphs }.normalized()
}

// ---- presentation ---------------------------------------------------------

/// The byte offset of each grapheme of a node's text and the unit it shows;
/// the breaks between paragraphs are not units.
pub fn unit_graphemes(text: &str) -> impl Iterator<Item = (usize, Option<u32>)> + '_ {
    let mut unit = 0;
    text.grapheme_indices(true).map(move |(byte, g)| {
        let this = (g != "\n").then(|| {
            unit += 1;
            unit - 1
        });
        (byte, this)
    })
}

/// Reveal times per grapheme of a node's text, on the session clock:
/// `Some(0)` already shown, `None` hidden.
#[derive(Debug, Clone)]
pub struct RevealPlan {
    pub at_ms: Vec<Option<u64>>,
    pub intro: Entrance,
}

impl RevealPlan {
    /// Map a [`Reveal`] over units onto the graphemes of `rich` (which was
    /// built from the same document for a display).
    pub fn new(rich: &RichText, reveal: &Reveal, intro: Entrance) -> Self {
        let mut last = Some(0);
        let at_ms = unit_graphemes(rich.text())
            .map(|(_, unit)| {
                if let Some(unit) = unit {
                    last = reveal.unit_at(unit);
                }
                last
            })
            .collect();
        Self { at_ms, intro }
    }

    /// The schedule relative to a document clock reading `anim_now`, taken
    /// at session time `now_ms`.
    pub fn schedule(&self, anim_now: Duration, now_ms: u64, reduced: bool) -> RevealSchedule {
        let at_s: Vec<f32> = self
            .at_ms
            .iter()
            .map(|at| match at {
                None => 1.0e6,
                Some(0) => -60.0,
                Some(t) => (*t as f64 - now_ms as f64) as f32 / 1000.0,
            })
            .collect();
        let secs = f32::from(self.intro.ms) / 1000.0;
        let intro = match self.intro.kind {
            _ if reduced => GlyphIntro::fade(0.0),
            EntranceKind::None => GlyphIntro::fade(0.0),
            EntranceKind::Fade => GlyphIntro::fade(secs),
            EntranceKind::Pop => GlyphIntro::pop(secs),
            EntranceKind::SlideUp => GlyphIntro::rise(10.0, secs),
        };
        RevealSchedule::new(anim_now, at_s).intro(intro)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look() -> Look {
        Look {
            size: 28.0,
            color: Rgba8::BLACK,
            weight: 500,
            line_height: 1.4,
            body: None,
            name: Some("Name Font".into()),
            accent: Some("Accent Font".into()),
            mention: MentionLook::default(),
        }
    }

    fn round_trip(markup: &str) {
        let doc = nanatype_markup::parse(markup).doc;
        let urls = HashMap::new();
        let look = look();
        let env = Env {
            look: &look,
            urls: &urls,
            editor: true,
            locale: Locale::ZhCn,
        };
        let rich = to_rich(&doc, &env);
        let back = from_rich(&rich, &look);
        assert_eq!(
            nanatype_markup::serialize(&back),
            nanatype_markup::serialize(&doc),
            "{markup}"
        );
    }

    #[test]
    fn documents_survive_the_editor() {
        round_trip("你好[b]粗[/b][i]斜[/i][u]下[/u][s]删[/s]");
        round_trip("[color=#e86a92]红[/color][size=1.5]大[/size][font=accent]装[/font]");
        round_trip("[stroke=#ffffff,2]描[/stroke][shadow=#000000aa,2,2,4]影[/shadow]");
        round_trip(
            "[shake]抖[/shake][wave amp=2]波[/wave][speed=0.5]慢[/speed][instant]快[/instant]",
        );
        round_trip("[ruby=かんじ]漢字[/ruby]と[b][ruby=よ]読[/ruby][/b]");
        round_trip("一[wait=500]二[pause]三[page]四[se=coin][expr=smile][screenshake=4,200]");
        round_trip("第一段\n第二段\n\n第四段");
        round_trip(
            "你好[at=42 name=观众 avatar=blob:bili-face-42]！[wave][at name=\"无 uid\"][/wave]",
        );
    }

    #[test]
    fn a_mention_is_one_label_object_with_its_avatar() {
        let doc = nanatype_markup::parse("[at=42 name=观众 avatar=blob:bili-face-42]").doc;
        let mut urls = HashMap::new();
        urls.insert(
            "blob:bili-face-42".to_owned(),
            "file:///f/42.png".to_owned(),
        );
        let look = look();
        let env = Env {
            look: &look,
            urls: &urls,
            editor: false,
            locale: Locale::ZhCn,
        };
        let rich = to_rich(&doc, &env);
        let (_, object) = &rich.objects()[0];
        assert_eq!(
            object.tag_label().map(|l| l.to_string()),
            Some("@观众".into())
        );
        let style = object.label_style().expect("a styled label");
        assert_eq!(style.icon.as_deref(), Some("file:///f/42.png"));
    }

    #[test]
    fn editor_markers_are_readable_chips_with_a_glyph() {
        let reg = nanatype_theme::SkinRegistry::new(None, "test");
        for e in reg.entries() {
            let skin = reg.load(&e.id).expect("skin loads").skin;
            let m = MarkerLook::from_skin(&skin);
            let r = crate::skin_theme::contrast(m.text, m.fill);
            assert!(r >= 4.5, "{}: marker chip ink {r:.2}:1", e.id);
        }
        let doc = nanatype_markup::parse("[wait=500][se=coin]").doc;
        let urls = HashMap::new();
        let look = look();
        let env = Env {
            look: &look,
            urls: &urls,
            editor: true,
            locale: Locale::ZhCn,
        };
        let rich = to_rich(&doc, &env);
        let icons: Vec<String> = rich
            .objects()
            .iter()
            .map(|(_, o)| {
                let icon = o.label_style().and_then(|s| s.icon.clone());
                icon.expect("a glyph").to_string()
            })
            .collect();
        assert_eq!(icons.len(), 2);
        assert!(icons.iter().all(|i| i.starts_with("data:image/svg+xml,")));
        assert_ne!(icons[0], icons[1], "each kind has its own glyph");
    }

    #[test]
    fn reveal_plans_follow_units_across_paragraphs() {
        let doc = Document::plain("ab\ncd");
        let urls = HashMap::new();
        let look = look();
        let env = Env {
            look: &look,
            urls: &urls,
            editor: false,
            locale: Locale::ZhCn,
        };
        let rich = to_rich(&doc, &env);
        let reveal = Reveal {
            visible_from: 0,
            shown: 1,
            segment: Some(nanatype_core::SegmentReveal {
                origin_ms: 1000,
                start: 1,
                reveal_ms: vec![0, 100, 200].into(),
            }),
            hidden_from: 4,
        };
        let plan = RevealPlan::new(
            &rich,
            &reveal,
            Entrance {
                kind: EntranceKind::Fade,
                ms: 100,
            },
        );
        assert_eq!(
            plan.at_ms,
            vec![Some(0), Some(1000), Some(1000), Some(1100), Some(1200)]
        );
    }
}
