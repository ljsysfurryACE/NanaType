//! Turn a theme pack into a NanaUI compiled theme. Colour numbers live in
//! `dialogue`; this file only installs them.

use std::sync::Arc;

use dialogue::{ThemePack, UiColor, UiPalette};
use nana_ui::theme::{
    CompiledTheme, SemanticColor, SemanticPalette, ThemeAppearance, ThemeDefinition, ThemeId,
};

pub fn compiled(pack: &ThemePack) -> Arc<CompiledTheme> {
    let appearance = if pack.ui.dark {
        ThemeAppearance::Dark
    } else {
        ThemeAppearance::Light
    };
    let id = match pack.id {
        dialogue::ThemeId::Halo => "nanatype.halo",
        dialogue::ThemeId::Night => "nanatype.night",
        dialogue::ThemeId::Parchment => "nanatype.parchment",
    };
    Arc::new(
        ThemeDefinition::for_appearance(appearance)
            .with_id(ThemeId::new(id))
            .with_display_name(pack.id.label())
            .with_palette(semantic(&pack.ui))
            .compile()
            .expect("内置主题包可以编译"),
    )
}

fn semantic(ui: &UiPalette) -> SemanticPalette {
    SemanticPalette {
        background: solid(ui.background),
        surface: solid(ui.surface),
        subtle: solid(ui.subtle),
        hover: solid(ui.hover),
        active: solid(ui.active),
        selected: solid(ui.selected),
        selected_hover: solid(ui.selected_hover),
        selected_pressed: solid(ui.selected_pressed),
        border: solid(ui.border),
        border_soft: solid(ui.border_soft),
        border_strong: solid(ui.border_strong),
        text: solid(ui.text),
        muted: solid(ui.muted),
        faint: solid(ui.faint),
        accent: solid(ui.accent),
        accent_strong: solid(ui.accent_strong),
        accent_soft: tint(ui.accent_soft),
        accent_soft_hover: tint(ui.accent_soft_hover),
        accent_soft_pressed: tint(ui.accent_soft_pressed),
        accent_on_soft: solid(ui.accent_on_soft),
        accent_text: solid(ui.accent_text),
        focus_surface: tint(ui.focus_surface),
        focus_border: solid(ui.focus_border),
        focus_text: solid(ui.focus_text),
        highlight: solid(ui.highlight),
        highlight_hover: solid(ui.highlight_hover),
        highlight_pressed: solid(ui.highlight_pressed),
        highlight_text: solid(ui.highlight_text),
        success: solid(ui.success),
        warning: solid(ui.warning),
        danger: solid(ui.danger),
    }
}

fn solid(color: UiColor) -> SemanticColor {
    SemanticColor::rgb8(color.r, color.g, color.b)
}

fn tint(color: UiColor) -> SemanticColor {
    SemanticColor::rgba8(color.r, color.g, color.b, color.a)
}
