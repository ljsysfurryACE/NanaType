//! Skins → what NanaUI paints with.
//!
//! The chrome part of a skin becomes a NanaUI theme for the toolbar, the
//! settings window and every built-in control; derived roles (hover, soft
//! accent, focus) are mixed from the few colours a skin states. User
//! overrides from the settings are applied on top of the skin first.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use nana_ui::CompiledTheme;
use nana_ui::theme::{AccentRamp, SemanticColor, SemanticPalette, ThemeDefinition, ThemeId};
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

pub fn compile(skin: &Skin, dark: bool) -> Arc<CompiledTheme> {
    let c = &skin.chrome;
    let skin_dark = c.appearance == Appearance::Dark;
    let mut p = if dark {
        SemanticPalette::dark()
    } else {
        SemanticPalette::light()
    };
    // Only take the skin's surfaces when it agrees with the chosen mode;
    // otherwise keep NanaUI's neutral surfaces and just carry the accent.
    if skin_dark == dark {
        let (bg, surface, text) = (c.background, c.surface, c.text);
        let toward = if dark { Rgba8::WHITE } else { Rgba8::BLACK };
        p.background = sc(bg);
        p.surface = sc(surface);
        p.subtle = sc(mix(surface, bg, 0.5));
        p.hover = sc(mix(surface, toward, 0.06));
        p.active = sc(mix(surface, toward, 0.12));
        p.border = sc(c.border);
        p.border_soft = sc(mix(c.border, surface, 0.5));
        p.border_strong = sc(mix(c.border, text, 0.3));
        p.text = sc(text);
        p.muted = sc(c.muted);
        p.faint = sc(mix(c.muted, surface, 0.35));
        p.selected = sc(mix(surface, c.accent, 0.16));
        p.selected_hover = sc(mix(surface, c.accent, 0.24));
        p.selected_pressed = sc(mix(surface, c.accent, 0.32));
        p.success = sc(c.success);
        p.warning = sc(c.warning);
        p.danger = sc(c.danger);
    }
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
    let accent = c.accent;
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
    let def = base
        .with_palette(p)
        .with_accent(ramp)
        .with_id(ThemeId::from_owned(format!(
            "nanatype.skin.{}.{}",
            skin.meta.id,
            if dark { "dark" } else { "light" }
        )))
        .with_display_name(skin.meta.id.clone());
    match def.compile() {
        Ok(theme) => Arc::new(theme),
        Err(_) => nana_ui::builtin_theme_arc(if dark {
            nana_ui::ThemeAppearance::Dark
        } else {
            nana_ui::ThemeAppearance::Light
        }),
    }
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
        assert!(bad.is_empty(), "
{}", bad.join("
"));
    }
}
