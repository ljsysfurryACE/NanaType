//! Skins → what NanaUI paints with.
//!
//! The chrome part of a skin becomes a NanaUI theme for the toolbar, the
//! settings window and every built-in control; derived roles (hover, soft
//! accent, focus) are mixed from the few colours a skin states. User
//! overrides from the settings are applied on top of the skin first.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use nana_ui::CompiledTheme;
use nana_ui::runtime::{BoxPaint, Gradient, PaintContext, PaintPath, PaintShadow, Painter};
use nana_ui::theme::{AccentRamp, SemanticColor, SemanticPalette, ThemeDefinition, ThemeId};
use nana_ui_core::{
    EffectTokens, ElevationRole, RadiusTier, SemanticColorRole, ShadowToken, ThemeAppearance,
    ThemeMetrics,
};
use nanatype_model::Rgba8;
use nanatype_settings::{ChromeMode, SkinOverrides};
use nanatype_theme::{Appearance, FontSpec, Skin};

/// The skin with the user's personalisation applied.
pub fn with_overrides(skin: &Skin, o: &SkinOverrides) -> Skin {
    let mut s = skin.clone();
    if let Some(c) = o.text_color {
        s.dialogue.text.color = c;
        s.history.text.color = c;
    }
    if let Some(c) = o.name_color {
        s.dialogue.nameplate.color = c;
        s.dialogue.nameplate.shape.gradient = None;
        s.history.name_color = c;
    }
    if let Some(c) = o.box_color {
        // A gradient would paint over the chosen colour.
        s.dialogue.box_.color = c;
        s.dialogue.box_.shape.gradient = None;
    }
    if let Some(c) = o.accent {
        s.chrome.accent = c;
        s.dialogue.continue_.color = c;
    }
    if let Some(size) = o.text_size {
        s.dialogue.text.size = size;
    }
    if let Some(region) = o.region {
        s.dialogue.region = region;
    }
    if let Some(overflow) = o.overflow {
        s.dialogue.overflow = overflow;
    }
    if o.overflow_auto_ms.is_some() {
        s.dialogue.overflow_auto_ms = o.overflow_auto_ms;
    }
    s
}

/// The skin as drawn: the user's personalisation applied, and each font
/// role's family resolved. `bundled` are the families registered from the
/// skin's own font files (body, name, accent), which always come first.
/// `language` is the UI locale tag: roles the skin leaves at the default
/// stack take that language's system stack, and roles with their own stack
/// keep it first and fall back to the language's faces after it.
pub fn prepare(
    base: &Skin,
    overrides: &SkinOverrides,
    bundled: &[Option<String>; 3],
    language: &str,
) -> Skin {
    let mut skin = with_overrides(base, overrides);
    let locale_stack = FontSpec::locale_families(language);
    for (spec, family) in skin.fonts.roles_mut().into_iter().zip(bundled) {
        if spec.is_default_stack() {
            spec.families = locale_stack.clone();
        } else {
            for f in &locale_stack {
                if !spec.families.iter().any(|g| g.eq_ignore_ascii_case(f)) {
                    spec.families.push(f.clone());
                }
            }
        }
        if let Some(family) = family {
            spec.families.insert(0, family.clone());
        }
        // NanaUI takes a single family name, so the fallback list is resolved
        // here: the first installed family goes first.
        if let Some(i) = spec.families.iter().position(|f| installed(f))
            && i > 0
        {
            let family = spec.families.remove(i);
            spec.families.insert(0, family);
        }
    }
    skin
}

/// Whether the text engine shapes with `family` itself rather than a
/// fallback. Cached: the answer only changes when fonts are installed.
fn installed(family: &str) -> bool {
    static CACHE: LazyLock<Mutex<HashMap<String, bool>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *cache.entry(family.to_owned()).or_insert_with(|| {
        nana_ui::shaped_face_families(family, "Aa字")
            .iter()
            .any(|f| f.eq_ignore_ascii_case(family))
    })
}

pub fn rgba(c: Rgba8) -> [f32; 4] {
    [
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
        f32::from(c.a) / 255.0,
    ]
}

fn sc(c: Rgba8) -> SemanticColor {
    SemanticColor::rgba8(c.r, c.g, c.b, f32::from(c.a) / 255.0)
}

fn mix(a: Rgba8, b: Rgba8, t: f32) -> Rgba8 {
    let m = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    Rgba8::rgba(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b), m(a.a, b.a))
}

fn luminance(c: Rgba8) -> f32 {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two opaque colours.
pub fn contrast(a: Rgba8, b: Rgba8) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The foreground that reads best on `bg`: the dark ink or white, whichever
/// has the higher contrast ratio. Mid-tone backgrounds where the soft ink
/// cannot reach 4.5:1 fall back to pure black, which always can.
pub fn on(bg: Rgba8) -> Rgba8 {
    let ink = Rgba8::rgb(16, 22, 32);
    let (dark, light) = (contrast(ink, bg), contrast(Rgba8::WHITE, bg));
    if dark < light {
        Rgba8::WHITE
    } else if dark >= 4.5 {
        ink
    } else {
        Rgba8::BLACK
    }
}

/// The accent as a foreground on `surface` (focus rings, soft-fill text,
/// slider and switch marks): the accent itself when it already reaches
/// `min`:1, otherwise mixed toward `text` just far enough that it does.
pub fn accent_fg(accent: Rgba8, surface: Rgba8, text: Rgba8, min: f32) -> Rgba8 {
    if contrast(accent, surface) >= min {
        return accent;
    }
    (1..=20)
        .map(|i| mix(accent, text, i as f32 / 20.0))
        .find(|c| contrast(*c, surface) >= min)
        .unwrap_or(text)
}

fn rgba8(c: SemanticColor) -> Rgba8 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba8::rgb(b(c.r), b(c.g), b(c.b))
}

pub fn dark_mode(skin: &Skin, mode: ChromeMode, system_dark: bool) -> bool {
    match mode {
        ChromeMode::Skin => skin.chrome.appearance == Appearance::Dark,
        ChromeMode::System => system_dark,
        ChromeMode::Light => false,
        ChromeMode::Dark => true,
    }
}

/// CIE L* (0–100) of an opaque colour: the lightness step a reader sees.
pub fn lightness(c: Rgba8) -> f32 {
    let y = luminance(c);
    if y > 216.0 / 24389.0 {
        116.0 * y.cbrt() - 16.0
    } else {
        y * 24389.0 / 27.0
    }
}

/// How far a colour is from grey, 0–1 (sRGB channel spread).
pub fn chroma(c: Rgba8) -> f32 {
    let hi = c.r.max(c.g).max(c.b);
    let lo = c.r.min(c.g).min(c.b);
    f32::from(hi - lo) / 255.0
}

/// The most a skin's chrome surface gives way to the neutral Nana surface.
pub const BLEND_MAX: f32 = 0.65;
/// The smallest CIE L* step between a selected row and the surface.
pub const SELECTED_DELTA_L: f32 = 6.0;

/// How much of a skin's chrome surface gives way to the neutral Nana
/// surface: nothing for a near-grey colour, up to [`BLEND_MAX`] for a
/// saturated one (pixel's lime), so a loud skin tints the window and keeps
/// its accent instead of washing the whole settings window in its colour.
fn blend_amount(c: Rgba8) -> f32 {
    ((chroma(c) - 0.08) / 0.25).clamp(0.0, 1.0) * BLEND_MAX
}

/// `fg` moved toward `toward` just far enough to reach `min`:1 on every
/// colour in `on`.
fn readable(fg: Rgba8, on: &[Rgba8], toward: Rgba8, min: f32) -> Rgba8 {
    let ok = |c: Rgba8| on.iter().all(|bg| contrast(c, *bg) >= min);
    if ok(fg) {
        return fg;
    }
    (1..=20)
        .map(|i| mix(fg, toward, i as f32 / 20.0))
        .find(|c| ok(*c))
        .unwrap_or(toward)
}

/// The chrome colours a theme is compiled from, for one appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeColors {
    pub dark: bool,
    pub accent: Rgba8,
    pub highlight: Rgba8,
    pub background: Rgba8,
    pub surface: Rgba8,
    pub text: Rgba8,
    pub muted: Rgba8,
    pub border: Rgba8,
    pub success: Rgba8,
    pub warning: Rgba8,
    pub danger: Rgba8,
    /// The selected-row wash's share of the accent: the least that moves
    /// the surface by [`SELECTED_DELTA_L`].
    pub selected_t: f32,
}

/// The skin's chrome for a chrome drawn `dark` or light:
///
/// 1. the skin's own colours when its appearance agrees, otherwise the Nana
///    counterpart ([`nanatype_theme::Chrome::nana`]) carrying the skin's
///    accent and highlight; `[chrome.dark]` / `[chrome.light]` on top;
/// 2. background and surface pulled toward the neutral Nana surface by
///    their saturation ([`blend_amount`]);
/// 3. text and muted lifted to 4.5:1, status colours to 3:1, on both.
pub fn chrome_colors(skin: &Skin, dark: bool) -> ChromeColors {
    let c = &skin.chrome;
    let nana = nanatype_theme::Chrome::nana(dark);
    let own = if (c.appearance == Appearance::Dark) == dark {
        c
    } else {
        &nana
    };
    let v = c.variant(dark).cloned().unwrap_or_default();
    let pick = |over: Option<Rgba8>, base: Rgba8| over.unwrap_or(base);
    let accent = pick(v.accent, c.accent);
    let highlight = pick(v.highlight, c.highlight_for(dark));
    let background = pick(v.background, own.background);
    let surface = pick(v.surface, own.surface);
    let background = mix(background, nana.background, blend_amount(background));
    let surface = mix(surface, nana.surface, blend_amount(surface));
    let extreme = if dark { Rgba8::WHITE } else { Rgba8::BLACK };
    let both = [surface, background];
    let text = readable(pick(v.text, own.text), &both, extreme, 4.5);
    let muted = readable(pick(v.muted, own.muted), &both, text, 4.5);
    let status = |over: Option<Rgba8>, base: Rgba8| readable(pick(over, base), &both, text, 3.0);
    let lsurface = lightness(surface);
    let selected_t = (16..=60)
        .map(|i| i as f32 / 100.0)
        .find(|t| (lightness(mix(surface, accent, *t)) - lsurface).abs() >= SELECTED_DELTA_L)
        .unwrap_or(0.6);
    ChromeColors {
        dark,
        accent,
        highlight,
        background,
        surface,
        text,
        muted,
        border: pick(v.border, own.border),
        success: status(v.success, own.success),
        warning: status(v.warning, own.warning),
        danger: status(v.danger, own.danger),
        selected_t,
    }
}

/// Radius steps 4 / 8 / 12 / 16 / 20 (and 24): swatches and tags, controls,
/// popovers, cards, floating rails, large surfaces. Chips are pills.
pub fn metrics() -> ThemeMetrics {
    ThemeMetrics {
        radius_xs: 4.0,
        radius_sm: 8.0,
        radius_md: 12.0,
        radius_lg: 16.0,
        radius_xl: 20.0,
        radius_xxl: 24.0,
        ..nana_ui_core::UI_METRICS
    }
}

/// The lifted-surface shadow cards and menus share: a soft blue-tinted
/// `0 8px 24px text@16%` on a light chrome, `0 10px 30px -12px #00000080`
/// on a dark one.
pub fn effects(k: &ChromeColors) -> EffectTokens {
    let (base, surface) = if k.dark {
        let shadow = ShadowToken {
            color: SemanticColor::rgba8(0, 0, 0, 0.5),
            offset_x: 0.0,
            offset_y: 10.0,
            blur_radius: 30.0,
            spread_radius: -12.0,
            inset: false,
        };
        (EffectTokens::DARK, shadow)
    } else {
        let t = k.text;
        let shadow = ShadowToken {
            color: SemanticColor::rgba8(t.r, t.g, t.b, 0.16),
            offset_x: 0.0,
            offset_y: 8.0,
            blur_radius: 24.0,
            spread_radius: 0.0,
            inset: false,
        };
        (EffectTokens::LIGHT, shadow)
    };
    EffectTokens { surface, ..base }
}

pub fn compile(skin: &Skin, dark: bool) -> Arc<CompiledTheme> {
    let c = &skin.chrome;
    let k = chrome_colors(skin, dark);
    let mut p = if dark {
        SemanticPalette::dark()
    } else {
        SemanticPalette::light()
    };
    let (bg, surface, text) = (k.background, k.surface, k.text);
    let toward = if dark { Rgba8::WHITE } else { Rgba8::BLACK };
    p.background = sc(bg);
    p.surface = sc(surface);
    p.subtle = sc(mix(surface, bg, 0.5));
    p.hover = sc(mix(surface, toward, 0.06));
    p.active = sc(mix(surface, toward, 0.12));
    p.border = sc(k.border);
    p.border_soft = sc(mix(k.border, surface, 0.5));
    p.border_strong = sc(mix(k.border, text, 0.3));
    p.text = sc(text);
    p.muted = sc(k.muted);
    p.faint = sc(mix(k.muted, surface, 0.35));
    p.selected = sc(mix(surface, k.accent, k.selected_t));
    p.selected_hover = sc(mix(surface, k.accent, k.selected_t + 0.08));
    p.selected_pressed = sc(mix(surface, k.accent, k.selected_t + 0.16));
    p.success = sc(k.success);
    p.warning = sc(k.warning);
    p.danger = sc(k.danger);
    // The warm second accent for live and attention states: a fill with its
    // own hover, pressed and on-fill text.
    let hl = k.highlight;
    p.highlight = sc(hl);
    p.highlight_hover = sc(mix(hl, toward, 0.12));
    p.highlight_pressed = sc(mix(hl, toward, 0.2));
    p.highlight_text = sc(on(hl));
    for (role, color) in &c.palette {
        let slot = match role.as_str() {
            "background" => &mut p.background,
            "surface" => &mut p.surface,
            "subtle" => &mut p.subtle,
            "hover" => &mut p.hover,
            "active" => &mut p.active,
            "selected" => &mut p.selected,
            "border" => &mut p.border,
            "text" => &mut p.text,
            "muted" => &mut p.muted,
            "faint" => &mut p.faint,
            "highlight" => &mut p.highlight,
            _ => continue,
        };
        *slot = sc(*color);
    }
    let accent = k.accent;
    let (surface, text) = (rgba8(p.surface), rgba8(p.text));
    let strong = mix(accent, if dark { Rgba8::WHITE } else { Rgba8::BLACK }, 0.15);
    // Text and marks drawn in the accent sit on a surface or on a soft
    // accent wash of it, so measure against the wash (its heaviest state).
    let wash = mix(surface, accent, 0.30);
    let fg_text = accent_fg(accent, wash, text, 4.5);
    let fg_ui = accent_fg(accent, surface, text, 3.0);
    let focus_alpha: u8 = if dark { 140 } else { 230 };
    let ramp = AccentRamp {
        base: sc(accent),
        strong: sc(strong),
        on_soft: sc(fg_text),
        text: sc(on(accent)),
        soft_alpha: 0.14,
        soft_hover_alpha: 0.22,
        soft_pressed_alpha: 0.30,
        focus: sc(accent.with_alpha(focus_alpha)),
    };
    // `with_accent` leaves the focus roles at NanaUI's own blue; carry the
    // skin through them. The ring is a solid `accent_fg`, the fill's label
    // is chosen against the fill as it lands on the surface.
    p.focus_border = sc(fg_ui);
    p.focus_surface = ramp.focus;
    p.focus_text = sc(on(mix(surface, accent, f32::from(focus_alpha) / 255.0)));
    let base = if dark {
        ThemeDefinition::NANA_DARK
    } else {
        ThemeDefinition::NANA_LIGHT
    };
    // Toasts arrive and leave on the app's motion tokens.
    let toast = nana_ui_core::ToastRecipe {
        motion: crate::ui::motion::toast(),
        ..base
            .components
            .toast
            .unwrap_or(nana_ui_core::ToastRecipe::DEFAULT)
    };
    let mut def = base
        .with_palette(p)
        .with_accent(ramp)
        .with_metrics(metrics())
        .with_effects(effects(&k))
        .with_toast(toast)
        .with_id(ThemeId::from_owned(format!(
            "nanatype.skin.{}.{}",
            skin.meta.id,
            if dark { "dark" } else { "light" }
        )))
        .with_display_name(skin.meta.id.clone());
    def.motion.hover_color_ms = crate::ui::motion::HOVER_MS as u16;
    match def.compile() {
        Ok(theme) => Arc::new(theme),
        Err(_) => nana_ui::builtin_theme_arc(if dark {
            nana_ui::ThemeAppearance::Dark
        } else {
            nana_ui::ThemeAppearance::Light
        }),
    }
}

/// `rgba` with its alpha replaced.
fn alpha(rgba: [f32; 4], a: f32) -> [f32; 4] {
    [rgba[0], rgba[1], rgba[2], a]
}

/// A settings card: the surface at the card radius on the theme's lifted
/// shadow. Light chromes drop the outline for the soft blue-tinted shadow;
/// dark chromes keep a 1px soft border, since a shadow barely reads on dark.
pub struct CardPainter;

impl Painter for CardPainter {
    fn paint(&self, cx: &mut PaintContext<'_>) {
        let rect = cx.bounds();
        let r = cx.radius(RadiusTier::Lg);
        let mut card = BoxPaint::fill(SemanticColorRole::Surface).shadow(ElevationRole::Surface);
        if cx.theme_appearance() == ThemeAppearance::Dark {
            card = card.border(SemanticColorRole::BorderSoft, 1.0);
        }
        cx.rounded_rect(rect, r, card);
    }

    fn paint_key(&self) -> u64 {
        0
    }
}

/// The right end of a call to action's fill (accent → accent.light): up to
/// 22% toward white, as far as the label keeps 4.5:1 on it.
fn accent_light(accent: [f32; 4]) -> [f32; 4] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let base = Rgba8::rgb(q(accent[0]), q(accent[1]), q(accent[2]));
    let label = on(base);
    let end = (0..=11)
        .rev()
        .map(|i| mix(base, Rgba8::WHITE, i as f32 * 0.02))
        .find(|c| contrast(label, *c) >= 4.5)
        .unwrap_or(base);
    alpha(rgba(end), accent[3])
}

/// The view's one primary action: a solid accent fill lightening to the
/// right (accent → accent.light at 90°) on an accent-tinted
/// `0 3px 8px accent@33%` shadow, its label in the on-accent colour.
pub struct CtaPainter;

impl Painter for CtaPainter {
    fn paint(&self, cx: &mut PaintContext<'_>) {
        let state = cx.state();
        if state.disabled {
            cx.draw_default();
            return;
        }
        let rect = cx.bounds();
        let r = cx.radius(RadiusTier::Sm);
        let mut path = PaintPath::new();
        path.rounded_rect(rect, [r; 4]);
        let role = if state.hovered || state.pressed {
            SemanticColorRole::AccentStrong
        } else {
            SemanticColorRole::Accent
        };
        let accent = cx.color(role);
        let dark = cx.theme_appearance() == ThemeAppearance::Dark;
        cx.shadow(
            &path,
            PaintShadow::Custom {
                color: alpha(accent, if dark { 0.25 } else { 0.33 }).into(),
                offset: [0.0, 3.0],
                blur: if dark { 10.0 } else { 8.0 },
                spread: 0.0,
                inset: false,
            },
        );
        let fill = Gradient::linear([rect.x, rect.y], [rect.x + rect.width, rect.y])
            .stop(0.0, accent)
            .stop(1.0, accent_light(accent));
        cx.fill_path(&path, fill);
        // The framework's own look over it: the label and the focus ring
        // (the node itself has no fill of its own).
        cx.draw_default();
    }

    fn paint_key(&self) -> u64 {
        1
    }
}

/// `button` as the view's call to action: [`CtaPainter`] over a solid accent
/// fill, its label in the on-accent colour.
pub fn cta(button: nana_ui::runtime::Button) -> nana_ui::runtime::Button {
    let mut st = button.style.clone();
    // The painter draws the fill, so the node has none of its own (a
    // disabled button shows the plain primary tint instead).
    st.foreground = Some(SemanticColorRole::AccentText);
    st.background = None;
    st.border = None;
    st.interaction.disabled.background = Some(SemanticColorRole::AccentSoft);
    st.painter = Some(CtaPainter.into());
    button.style(st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanatype_theme::{SkinOrigin, SkinRegistry};

    #[test]
    fn on_accent_text_reaches_4_5_for_every_builtin_skin() {
        let reg = SkinRegistry::new(None, "test");
        let mut n = 0;
        for e in reg
            .entries()
            .iter()
            .filter(|e| matches!(e.origin, SkinOrigin::Builtin))
        {
            let skin = reg.load(&e.id).expect("builtin skin loads").skin;
            let accent = skin.chrome.accent;
            let r = contrast(on(accent), accent);
            assert!(r >= 4.5, "{}: on(accent) is {r:.2}:1", e.id);
            n += 1;
        }
        assert_eq!(n, 18);
    }

    #[test]
    fn accent_fg_meets_its_minimum_on_the_skin_surface() {
        let reg = SkinRegistry::new(None, "test");
        for e in reg.entries() {
            let c = reg.load(&e.id).expect("skin loads").skin.chrome.clone();
            for min in [3.0, 4.5] {
                let fg = accent_fg(c.accent, c.surface, c.text, min);
                let r = contrast(fg, c.surface);
                assert!(r >= min, "{}: accent_fg {r:.2}:1 < {min}", e.id);
            }
        }
    }

    fn builtins() -> Vec<(String, Skin)> {
        let reg = SkinRegistry::new(None, "test");
        reg.entries()
            .iter()
            .filter(|e| matches!(e.origin, SkinOrigin::Builtin))
            .map(|e| {
                let skin = reg.load(&e.id).expect("builtin skin loads").skin;
                (e.id.clone(), (*skin).clone())
            })
            .collect()
    }

    #[test]
    fn resolved_chrome_is_readable_in_both_modes_for_every_builtin_skin() {
        let skins = builtins();
        assert_eq!(skins.len(), 18);
        let mut bad = Vec::new();
        for (id, skin) in &skins {
            for dark in [false, true] {
                let k = chrome_colors(skin, dark);
                for (on_name, bg) in [("surface", k.surface), ("background", k.background)] {
                    for (role, fg, min) in [
                        ("text", k.text, 4.5),
                        ("muted", k.muted, 4.5),
                        ("success", k.success, 3.0),
                        ("warning", k.warning, 3.0),
                        ("danger", k.danger, 3.0),
                    ] {
                        let r = contrast(fg, bg);
                        if r < min {
                            bad.push(format!("{id} dark={dark} {role}/{on_name} {r:.2}:1"));
                        }
                    }
                }
                let selected = mix(k.surface, k.accent, k.selected_t);
                let dl = (lightness(selected) - lightness(k.surface)).abs();
                if dl < SELECTED_DELTA_L {
                    bad.push(format!("{id} dark={dark} selected ΔL* {dl:.1}"));
                }
                let fg = accent_fg(k.accent, k.surface, k.text, 3.0);
                if contrast(fg, k.surface) < 3.0 {
                    bad.push(format!("{id} dark={dark} accent_fg"));
                }
                if contrast(on(k.highlight), k.highlight) < 4.5 {
                    bad.push(format!("{id} dark={dark} on(highlight)"));
                }
            }
        }
        assert!(bad.is_empty(), "\n{}", bad.join("\n"));
    }

    #[test]
    fn saturated_skin_chrome_is_tinted_not_washed() {
        let skins = builtins();
        let pixel = &skins
            .iter()
            .find(|(id, _)| id == "nanatype.pixel")
            .unwrap()
            .1;
        let k = chrome_colors(pixel, false);
        // The lime page becomes a tint: well under half its saturation.
        assert!(chroma(k.background) < chroma(pixel.chrome.background) * 0.5);
        // The skin's accent is carried as is.
        assert_eq!(k.accent, pixel.chrome.accent);
        // Near-grey chromes are left alone.
        let halo = &skins
            .iter()
            .find(|(id, _)| id == "nanatype.halo")
            .unwrap()
            .1;
        let k = chrome_colors(halo, false);
        assert_eq!(k.background, halo.chrome.background);
        assert_eq!(k.surface, halo.chrome.surface);
    }

    #[test]
    fn a_mode_the_skin_does_not_match_uses_the_nana_counterpart() {
        let skins = builtins();
        let sakura = &skins
            .iter()
            .find(|(id, _)| id == "nanatype.sakura")
            .unwrap()
            .1;
        let k = chrome_colors(sakura, true);
        assert_eq!(k.background, Rgba8::rgb(15, 23, 36));
        assert_eq!(k.surface, Rgba8::rgb(24, 35, 58));
        assert_eq!(k.text, Rgba8::rgb(230, 238, 248));
        assert_eq!(k.accent, sakura.chrome.accent);
        assert_eq!(k.highlight, nanatype_theme::HIGHLIGHT_DARK);
        let night = &skins
            .iter()
            .find(|(id, _)| id == "nanatype.night")
            .unwrap()
            .1;
        let k = chrome_colors(night, false);
        assert_eq!(k.surface, nanatype_theme::Chrome::default().surface);
        // A `[chrome.dark]` table wins over the counterpart.
        let mut s = sakura.clone();
        s.chrome.dark = Some(nanatype_theme::ChromeVariant {
            surface: Some(Rgba8::rgb(40, 20, 30)),
            ..Default::default()
        });
        assert_eq!(chrome_colors(&s, true).surface, Rgba8::rgb(40, 20, 30));
    }

    #[test]
    fn builtin_chrome_text_roles_are_readable() {
        let reg = SkinRegistry::new(None, "test");
        let mut bad = Vec::new();
        for e in reg.entries() {
            let c = reg.load(&e.id).expect("skin loads").skin.chrome.clone();
            for (role, fg, min) in [
                ("text", c.text, 4.5),
                ("muted", c.muted, 4.5),
                ("success", c.success, 3.0),
                ("warning", c.warning, 3.0),
                ("danger", c.danger, 3.0),
            ] {
                for (on_name, bg) in [("surface", c.surface), ("background", c.background)] {
                    let r = contrast(fg, bg);
                    if r < min {
                        bad.push(format!("{} {role} on {on_name}: {r:.2}:1 < {min}", e.id));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "
{}",
            bad.join(
                "
"
            )
        );
    }
}
