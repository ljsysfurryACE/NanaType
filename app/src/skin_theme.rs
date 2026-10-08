//! Skins → what NanaUI paints with.
//!
//! The chrome part of a skin becomes a NanaUI theme for the toolbar, the
//! settings window and every built-in control; derived roles (hover, soft
//! accent, focus) are mixed from the few colours a skin states. User
//! overrides from the settings are applied on top of the skin first.

use std::sync::Arc;

use nana_ui::CompiledTheme;
use nana_ui::theme::{AccentRamp, SemanticColor, SemanticPalette, ThemeDefinition, ThemeId};
use nanatype_model::Rgba8;
use nanatype_settings::{ChromeMode, SkinOverrides};
use nanatype_theme::{Appearance, Skin};

/// The skin with the user's personalisation applied.
pub fn with_overrides(skin: &Skin, o: &SkinOverrides) -> Skin {
    let mut s = skin.clone();
    if let Some(c) = o.text_color {
        s.dialogue.text.color = c;
        s.history.text.color = c;
    }
    if let Some(c) = o.name_color {
        s.dialogue.nameplate.color = c;
        s.history.name_color = c;
    }
    if let Some(c) = o.box_color {
        s.dialogue.box_.color = c;
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
    s
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

/// Black or white, whichever reads better on `bg`.
pub fn on(bg: Rgba8) -> Rgba8 {
    if luminance(bg) > 0.45 {
        Rgba8::rgb(16, 22, 32)
    } else {
        Rgba8::WHITE
    }
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
    let strong = mix(accent, if dark { Rgba8::WHITE } else { Rgba8::BLACK }, 0.15);
    let ramp = AccentRamp {
        base: sc(accent),
        strong: sc(strong),
        on_soft: sc(mix(
            accent,
            if dark { Rgba8::WHITE } else { Rgba8::BLACK },
            0.35,
        )),
        text: sc(on(accent)),
        soft_alpha: 0.14,
        soft_hover_alpha: 0.22,
        soft_pressed_alpha: 0.30,
        focus: sc(accent.with_alpha(if dark { 140 } else { 230 })),
    };
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
