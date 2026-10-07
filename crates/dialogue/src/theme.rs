//! Three built-in packs. One pack paints settings, the toolbar and the dialogue.
//! Fonts are part of the pack; settings do not offer a font picker.
//!
//! Halo and Night copy NanaLive's sky-blue palette numbers. Parchment is the
//! warm sheet that sits beside them. Nothing here depends on NanaLive.

use serde::{Deserialize, Serialize};

/// Performance motion drawn per grapheme. `None` is still text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Motion {
    #[default]
    None,
    Shake,
    Wave,
    Jump,
    Rainbow,
}

impl Motion {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "静止",
            Self::Shake => "抖动",
            Self::Wave => "波浪",
            Self::Jump => "跳跃",
            Self::Rainbow => "彩虹",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeId {
    Halo,
    Night,
    Parchment,
}

impl ThemeId {
    pub fn all() -> [Self; 3] {
        [Self::Halo, Self::Night, Self::Parchment]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Halo => "光环",
            Self::Night => "夜色",
            Self::Parchment => "羊皮纸",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Halo => "halo",
            Self::Night => "night",
            Self::Parchment => "parchment",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "night" => Self::Night,
            "parchment" => Self::Parchment,
            _ => Self::Halo,
        }
    }
}

impl Default for ThemeId {
    fn default() -> Self {
        Self::Halo
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl UiColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

/// Semantic roles the settings window installs into NanaUI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiPalette {
    pub dark: bool,
    pub background: UiColor,
    pub surface: UiColor,
    pub subtle: UiColor,
    pub hover: UiColor,
    pub active: UiColor,
    pub selected: UiColor,
    pub selected_hover: UiColor,
    pub selected_pressed: UiColor,
    pub border: UiColor,
    pub border_soft: UiColor,
    pub border_strong: UiColor,
    pub text: UiColor,
    pub muted: UiColor,
    pub faint: UiColor,
    pub accent: UiColor,
    pub accent_strong: UiColor,
    pub accent_soft: UiColor,
    pub accent_soft_hover: UiColor,
    pub accent_soft_pressed: UiColor,
    pub accent_on_soft: UiColor,
    pub accent_text: UiColor,
    pub focus_surface: UiColor,
    pub focus_border: UiColor,
    pub focus_text: UiColor,
    pub highlight: UiColor,
    pub highlight_hover: UiColor,
    pub highlight_pressed: UiColor,
    pub highlight_text: UiColor,
    pub success: UiColor,
    pub warning: UiColor,
    pub danger: UiColor,
}

/// Height of the input tool capsule. It sits above the dialogue and is not
/// part of the dialogue aspect ratio.
pub const INPUT_TOOLBAR_PX: f32 = 104.0;

/// Dialogue frame shared by the input stage and the typewriter client.
/// Fonts, the card inset and the text padding all live here so neither window
/// invents its own width, height or type size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogueChrome {
    pub font_family: &'static str,
    pub font_weight: u16,
    pub body_px: f32,
    pub name_px: f32,
    pub serif: bool,
    pub name_bg: [u8; 4],
    pub name_fg: [u8; 4],
    pub body_bg: [u8; 4],
    pub body_fg: [u8; 4],
    pub body_border: [u8; 4],
    pub radius: f32,
    pub glow: bool,
    /// Logical size of the dialogue region. The input stage and the typewriter
    /// client are this rectangle; the tool capsule is extra, above it.
    pub region_w: f32,
    pub region_h: f32,
    /// Gap between the region edge and the rounded card. Kept small so the
    /// typewriter client is almost the card itself.
    pub frame_inset: f32,
    pub content_pad_x: f32,
    pub content_pad_y: f32,
    /// Horizontal padding inside the name plate, and the gap under it.
    pub name_pad_x: f32,
    pub name_pad_y: f32,
    pub name_gap: f32,
}

impl DialogueChrome {
    /// One dialogue rectangle for every theme. Switching packs changes paint,
    /// not the aspect ratio shared by the input stage and the typewriter.
    pub const REGION_W: f32 = 760.0;
    pub const REGION_H: f32 = 200.0;

    /// Card rectangle inside a dialogue region, in logical pixels.
    pub fn card(self, width: f32, height: f32, pop: f32) -> (f32, f32, f32, f32) {
        let pop = pop.clamp(0.92, 1.0);
        let inset = self.frame_inset;
        let box_w = (width - inset * 2.0).max(40.0) * pop;
        let box_h = (height - inset * 2.0).max(40.0) * pop;
        let box_x = (width - box_w) / 2.0;
        let box_y = (height - box_h) / 2.0;
        (box_x, box_y, box_w, box_h)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePack {
    pub id: ThemeId,
    pub ui: UiPalette,
    pub chrome: DialogueChrome,
}

impl ThemePack {
    pub fn get(id: ThemeId) -> Self {
        match id {
            ThemeId::Halo => Self::halo(),
            ThemeId::Night => Self::night(),
            ThemeId::Parchment => Self::parchment(),
        }
    }

    pub fn halo() -> Self {
        Self {
            id: ThemeId::Halo,
            ui: halo_ui(),
            chrome: DialogueChrome {
                font_family: "Microsoft YaHei",
                font_weight: 500,
                body_px: 28.0,
                name_px: 18.0,
                serif: false,
                name_bg: [8, 118, 208, 255],
                name_fg: [255, 255, 255, 255],
                body_bg: [255, 255, 255, 255],
                body_fg: [27, 47, 75, 255],
                body_border: [203, 224, 239, 255],
                radius: 18.0,
                glow: false,
                region_w: DialogueChrome::REGION_W,
                region_h: DialogueChrome::REGION_H,
                frame_inset: 4.0,
                content_pad_x: 16.0,
                content_pad_y: 12.0,
                name_pad_x: 8.0,
                name_pad_y: 4.0,
                name_gap: 8.0,
            },
        }
    }

    pub fn night() -> Self {
        Self {
            id: ThemeId::Night,
            ui: night_ui(),
            chrome: DialogueChrome {
                font_family: "Microsoft YaHei",
                font_weight: 500,
                body_px: 28.0,
                name_px: 18.0,
                serif: false,
                name_bg: [92, 184, 255, 255],
                name_fg: [11, 26, 42, 255],
                body_bg: [16, 28, 44, 230],
                body_fg: [186, 224, 255, 255],
                body_border: [92, 184, 255, 255],
                radius: 18.0,
                glow: true,
                region_w: DialogueChrome::REGION_W,
                region_h: DialogueChrome::REGION_H,
                frame_inset: 4.0,
                content_pad_x: 16.0,
                content_pad_y: 12.0,
                name_pad_x: 8.0,
                name_pad_y: 4.0,
                name_gap: 8.0,
            },
        }
    }

    pub fn parchment() -> Self {
        Self {
            id: ThemeId::Parchment,
            ui: parchment_ui(),
            chrome: DialogueChrome {
                font_family: "SimSun",
                font_weight: 600,
                body_px: 30.0,
                name_px: 20.0,
                serif: true,
                name_bg: [92, 52, 28, 255],
                name_fg: [255, 244, 220, 255],
                body_bg: [244, 228, 196, 255],
                body_fg: [62, 42, 28, 255],
                body_border: [122, 84, 48, 255],
                radius: 6.0,
                glow: false,
                region_w: DialogueChrome::REGION_W,
                region_h: DialogueChrome::REGION_H,
                frame_inset: 4.0,
                content_pad_x: 16.0,
                content_pad_y: 12.0,
                name_pad_x: 8.0,
                name_pad_y: 4.0,
                name_gap: 8.0,
            },
        }
    }
}

fn halo_ui() -> UiPalette {
    // NanaLive light 「光环」. Sky blue (30, 155, 245) is the soft-accent source.
    const SKY: (u8, u8, u8) = (30, 155, 245);
    UiPalette {
        dark: false,
        background: UiColor::rgb(234, 244, 251),
        surface: UiColor::rgb(255, 255, 255),
        subtle: UiColor::rgb(243, 249, 253),
        hover: UiColor::rgb(221, 238, 250),
        active: UiColor::rgb(181, 217, 242),
        selected: UiColor::rgb(211, 236, 253),
        selected_hover: UiColor::rgb(190, 226, 251),
        selected_pressed: UiColor::rgb(165, 212, 245),
        border: UiColor::rgb(203, 224, 239),
        border_soft: UiColor::rgb(225, 238, 248),
        border_strong: UiColor::rgb(159, 191, 216),
        text: UiColor::rgb(27, 47, 75),
        muted: UiColor::rgb(74, 98, 128),
        faint: UiColor::rgb(127, 149, 174),
        accent: UiColor::rgb(8, 118, 208),
        accent_strong: UiColor::rgb(7, 96, 176),
        accent_soft: UiColor::rgba(SKY.0, SKY.1, SKY.2, 0.12),
        accent_soft_hover: UiColor::rgba(SKY.0, SKY.1, SKY.2, 0.20),
        accent_soft_pressed: UiColor::rgba(SKY.0, SKY.1, SKY.2, 0.29),
        accent_on_soft: UiColor::rgb(9, 80, 142),
        accent_text: UiColor::rgb(255, 255, 255),
        focus_surface: UiColor::rgba(7, 96, 176, 0.90),
        focus_border: UiColor::rgb(7, 96, 176),
        focus_text: UiColor::rgb(255, 255, 255),
        highlight: UiColor::rgb(14, 150, 200),
        highlight_hover: UiColor::rgb(10, 132, 180),
        highlight_pressed: UiColor::rgb(8, 114, 158),
        highlight_text: UiColor::rgb(255, 255, 255),
        success: UiColor::rgb(10, 128, 92),
        warning: UiColor::rgb(163, 100, 0),
        danger: UiColor::rgb(214, 52, 88),
    }
}

fn night_ui() -> UiPalette {
    UiPalette {
        dark: true,
        background: UiColor::rgb(14, 18, 24),
        surface: UiColor::rgb(28, 37, 49),
        subtle: UiColor::rgb(23, 31, 41),
        hover: UiColor::rgb(40, 52, 68),
        active: UiColor::rgb(56, 72, 92),
        selected: UiColor::rgb(36, 62, 92),
        selected_hover: UiColor::rgb(45, 77, 111),
        selected_pressed: UiColor::rgb(30, 52, 78),
        border: UiColor::rgb(64, 79, 98),
        border_soft: UiColor::rgb(45, 57, 72),
        border_strong: UiColor::rgb(96, 114, 137),
        text: UiColor::rgb(230, 237, 245),
        muted: UiColor::rgb(166, 181, 199),
        faint: UiColor::rgb(132, 149, 169),
        accent: UiColor::rgb(92, 184, 255),
        accent_strong: UiColor::rgb(64, 160, 240),
        accent_soft: UiColor::rgba(92, 184, 255, 0.16),
        accent_soft_hover: UiColor::rgba(92, 184, 255, 0.24),
        accent_soft_pressed: UiColor::rgba(92, 184, 255, 0.32),
        accent_on_soft: UiColor::rgb(169, 216, 255),
        accent_text: UiColor::rgb(11, 26, 42),
        focus_surface: UiColor::rgba(92, 184, 255, 0.54),
        focus_border: UiColor::rgb(92, 184, 255),
        focus_text: UiColor::rgb(255, 255, 255),
        highlight: UiColor::rgb(52, 160, 200),
        highlight_hover: UiColor::rgb(72, 178, 214),
        highlight_pressed: UiColor::rgb(40, 142, 180),
        highlight_text: UiColor::rgb(11, 26, 42),
        success: UiColor::rgb(72, 210, 150),
        warning: UiColor::rgb(255, 200, 90),
        danger: UiColor::rgb(255, 122, 150),
    }
}

fn parchment_ui() -> UiPalette {
    const INK: (u8, u8, u8) = (160, 100, 48);
    UiPalette {
        dark: false,
        background: UiColor::rgb(250, 244, 232),
        surface: UiColor::rgb(255, 252, 245),
        subtle: UiColor::rgb(245, 236, 220),
        hover: UiColor::rgb(236, 224, 204),
        active: UiColor::rgb(214, 196, 168),
        selected: UiColor::rgb(232, 214, 184),
        selected_hover: UiColor::rgb(220, 198, 164),
        selected_pressed: UiColor::rgb(200, 176, 140),
        border: UiColor::rgb(214, 196, 168),
        border_soft: UiColor::rgb(232, 220, 200),
        border_strong: UiColor::rgb(168, 140, 108),
        text: UiColor::rgb(62, 42, 28),
        muted: UiColor::rgb(110, 84, 62),
        faint: UiColor::rgb(150, 126, 102),
        accent: UiColor::rgb(122, 72, 36),
        accent_strong: UiColor::rgb(96, 54, 24),
        accent_soft: UiColor::rgba(INK.0, INK.1, INK.2, 0.16),
        accent_soft_hover: UiColor::rgba(INK.0, INK.1, INK.2, 0.24),
        accent_soft_pressed: UiColor::rgba(INK.0, INK.1, INK.2, 0.32),
        accent_on_soft: UiColor::rgb(96, 48, 16),
        accent_text: UiColor::rgb(255, 248, 236),
        focus_surface: UiColor::rgba(96, 54, 24, 0.90),
        focus_border: UiColor::rgb(96, 54, 24),
        focus_text: UiColor::rgb(255, 248, 236),
        highlight: UiColor::rgb(176, 112, 48),
        highlight_hover: UiColor::rgb(150, 92, 36),
        highlight_pressed: UiColor::rgb(122, 72, 28),
        highlight_text: UiColor::rgb(255, 248, 236),
        success: UiColor::rgb(46, 110, 62),
        warning: UiColor::rgb(160, 96, 20),
        danger: UiColor::rgb(168, 48, 42),
    }
}
