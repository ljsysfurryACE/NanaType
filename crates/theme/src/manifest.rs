//! `skin.toml`. Every table and field has a default, so a skin only states
//! what differs from its `base`. Inheritance is a deep merge of TOML tables
//! before this schema is applied (see [`crate::load`]).

use std::collections::BTreeMap;

use nanatype_model::{EntranceKind, Rgba8};
use nanatype_playback::{PacingParams, PunctPauses, TypingSoundRule};
use serde::{Deserialize, Serialize};

pub const SKIN_SCHEMA: u32 = 1;

/// Localized display strings keyed by BCP-47 tag (`zh-CN`, `en-US`, `ja-JP`).
pub type Localized = BTreeMap<String, String>;

pub fn localized(map: &Localized, locale: &str, fallback: &str) -> String {
    map.get(locale)
        .or_else(|| {
            let lang = locale.split('-').next().unwrap_or(locale);
            map.iter()
                .find(|(k, _)| k.split('-').next() == Some(lang))
                .map(|(_, v)| v)
        })
        .or_else(|| map.get("en-US"))
        .or_else(|| map.values().next())
        .cloned()
        .unwrap_or_else(|| fallback.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// A nine-slice image: `slice` is `[top, right, bottom, left]` in image pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NineSlice {
    pub image: String,
    pub slice: [u32; 4],
    /// Drawn size of the slice borders as a multiple of `slice`.
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub repeat: bool,
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextShadowSpec {
    pub color: Rgba8,
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
    #[serde(default)]
    pub blur: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStrokeSpec {
    pub color: Rgba8,
    pub width: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub schema: u32,
    pub id: String,
    pub version: String,
    /// Oldest NanaType version that understands this skin.
    pub min_app: String,
    /// Id of the skin this one extends.
    pub base: Option<String>,
    pub name: Localized,
    pub description: Localized,
    pub author: String,
    pub license: String,
    pub preview: Option<String>,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            schema: SKIN_SCHEMA,
            id: String::new(),
            version: "1.0.0".into(),
            min_app: "0.1.0".into(),
            base: None,
            name: Localized::new(),
            description: Localized::new(),
            author: String::new(),
            license: String::new(),
            preview: None,
        }
    }
}

/// Colours for the app's own controls (settings, toolbar). Roles left out
/// derive from `accent`, `surface` and `text`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chrome {
    pub appearance: Appearance,
    pub accent: Rgba8,
    pub background: Rgba8,
    pub surface: Rgba8,
    pub text: Rgba8,
    pub muted: Rgba8,
    pub border: Rgba8,
    pub success: Rgba8,
    pub warning: Rgba8,
    pub danger: Rgba8,
    /// Any other `SemanticPalette` role by name, for full control.
    pub palette: BTreeMap<String, Rgba8>,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            appearance: Appearance::Light,
            accent: Rgba8::rgb(8, 118, 208),
            background: Rgba8::rgb(234, 244, 251),
            surface: Rgba8::rgb(255, 255, 255),
            text: Rgba8::rgb(27, 47, 75),
            muted: Rgba8::rgb(74, 98, 128),
            border: Rgba8::rgb(203, 224, 239),
            success: Rgba8::rgb(10, 128, 92),
            warning: Rgba8::rgb(163, 100, 0),
            danger: Rgba8::rgb(214, 52, 88),
            palette: BTreeMap::new(),
        }
    }
}

/// A font role: a packaged file and/or system families, tried in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FontSpec {
    pub file: Option<String>,
    pub families: Vec<String>,
    pub weight: u16,
}

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            file: None,
            families: vec![
                "Microsoft YaHei UI".into(),
                "PingFang SC".into(),
                "Noto Sans SC".into(),
            ],
            weight: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Fonts {
    pub body: FontSpec,
    pub name: FontSpec,
    pub accent: FontSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoxSpec {
    pub color: Rgba8,
    pub border_color: Rgba8,
    pub border_width: f32,
    /// A second border inside the first (RPG windows).
    pub inner_border_color: Option<Rgba8>,
    pub inner_border_width: f32,
    pub radius: f32,
    pub glow: Option<Rgba8>,
    pub glow_blur: f32,
    pub shadow: Option<TextShadowSpec>,
    /// Replaces the drawn box when present.
    pub frame: Option<NineSlice>,
    /// Box width as a fraction of the region (ADV boxes span the screen).
    pub width_fraction: f32,
}

impl Default for BoxSpec {
    fn default() -> Self {
        Self {
            color: Rgba8::rgb(255, 255, 255),
            border_color: Rgba8::rgb(203, 224, 239),
            border_width: 1.0,
            inner_border_color: None,
            inner_border_width: 0.0,
            radius: 18.0,
            glow: None,
            glow_blur: 12.0,
            shadow: None,
            frame: None,
            width_fraction: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextSpec {
    pub color: Rgba8,
    pub size: f32,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub stroke: Option<TextStrokeSpec>,
    pub shadow: Option<TextShadowSpec>,
}

impl Default for TextSpec {
    fn default() -> Self {
        Self {
            color: Rgba8::rgb(27, 47, 75),
            size: 28.0,
            line_height: 1.4,
            letter_spacing: 0.0,
            stroke: None,
            shadow: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NameplateSpec {
    pub visible: bool,
    pub color: Rgba8,
    pub text_color: Rgba8,
    pub size: f32,
    pub radius: f32,
    pub padding: [f32; 2],
    /// Offset of the plate from the box's top-left corner; negative `y`
    /// lifts it onto the border like most ADV skins.
    pub offset: [f32; 2],
    pub frame: Option<NineSlice>,
    pub stroke: Option<TextStrokeSpec>,
}

impl Default for NameplateSpec {
    fn default() -> Self {
        Self {
            visible: true,
            color: Rgba8::rgb(8, 118, 208),
            text_color: Rgba8::rgb(255, 255, 255),
            size: 18.0,
            radius: 10.0,
            padding: [10.0, 4.0],
            offset: [16.0, -12.0],
            frame: None,
            stroke: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PortraitSpec {
    pub visible: bool,
    pub side: Side,
    pub size: [f32; 2],
    pub offset: [f32; 2],
    /// Image used when no variant is active, or when the active one is missing.
    pub image: Option<String>,
    /// Variant name → image. Expression markers switch to the variant of the
    /// same name.
    pub variants: BTreeMap<String, String>,
    pub radius: f32,
}

impl Default for PortraitSpec {
    fn default() -> Self {
        Self {
            visible: false,
            side: Side::Left,
            size: [160.0, 200.0],
            offset: [0.0, 0.0],
            image: None,
            variants: BTreeMap::new(),
            radius: 12.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContinueSpec {
    /// A glyph drawn when no image is given.
    pub glyph: String,
    pub color: Rgba8,
    pub size: f32,
    /// Sprite sheet laid out horizontally.
    pub image: Option<String>,
    pub frames: u32,
    pub fps: f32,
    pub anchor: Anchor,
    pub bob_px: f32,
}

impl Default for ContinueSpec {
    fn default() -> Self {
        Self {
            glyph: "▼".into(),
            color: Rgba8::rgb(8, 118, 208),
            size: 16.0,
            image: None,
            frames: 1,
            fps: 8.0,
            anchor: Anchor::BottomRight,
            bob_px: 3.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceSpec {
    pub kind: EntranceKind,
    pub ms: u16,
}

impl Default for EntranceSpec {
    fn default() -> Self {
        Self {
            kind: EntranceKind::Fade,
            ms: 140,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RevealSpec {
    pub base_ms: u16,
    pub cjk_factor: f32,
    pub space_ms: u16,
    pub punct: PunctPauses,
    pub entrance: EntranceSpec,
}

impl Default for RevealSpec {
    fn default() -> Self {
        let p = PacingParams::default();
        Self {
            base_ms: p.base_ms,
            cjk_factor: p.cjk_factor,
            space_ms: p.space_ms,
            punct: p.punct,
            entrance: EntranceSpec::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dialogue {
    /// Logical size of the dialogue region; the display window and the input
    /// stage are this size (plus `overscan` on every side).
    pub region: [f32; 2],
    /// Transparent margin so jumping, shaking and glowing text is not clipped.
    pub overscan: f32,
    /// Text inset inside the box: `[left, top, right, bottom]`.
    pub padding: [f32; 4],
    #[serde(rename = "box")]
    pub box_: BoxSpec,
    pub text: TextSpec,
    pub nameplate: NameplateSpec,
    pub portrait: PortraitSpec,
    #[serde(rename = "continue")]
    pub continue_: ContinueSpec,
    pub reveal: RevealSpec,
    /// Background drawn behind the box inside the region (rarely used).
    pub background: Option<String>,
}

impl Default for Dialogue {
    fn default() -> Self {
        Self {
            region: [760.0, 200.0],
            overscan: 18.0,
            padding: [20.0, 22.0, 20.0, 14.0],
            box_: BoxSpec::default(),
            text: TextSpec::default(),
            nameplate: NameplateSpec::default(),
            portrait: PortraitSpec::default(),
            continue_: ContinueSpec::default(),
            reveal: RevealSpec::default(),
            background: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarShape {
    #[default]
    Circle,
    Square,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryAvatar {
    pub shape: AvatarShape,
    pub size: f32,
}

impl Default for HistoryAvatar {
    fn default() -> Self {
        Self {
            shape: AvatarShape::Circle,
            size: 40.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryEntrance {
    None,
    #[default]
    SlideUp,
    Fade,
    SlideLeft,
}

/// The blivechat-style list: avatar, name, then a bubble per line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub width: f32,
    pub height: f32,
    pub avatar: HistoryAvatar,
    pub name_color: Rgba8,
    pub name_size: f32,
    pub name_stroke: Option<TextStrokeSpec>,
    pub bubble_color: Rgba8,
    pub bubble_border: Option<Rgba8>,
    pub bubble_radius: f32,
    pub bubble_padding: [f32; 2],
    pub bubble_frame: Option<NineSlice>,
    pub text: TextSpec,
    pub gap: f32,
    pub entrance: HistoryEntrance,
    pub show_time: bool,
    pub time_color: Rgba8,
}

impl Default for History {
    fn default() -> Self {
        Self {
            width: 420.0,
            height: 560.0,
            avatar: HistoryAvatar::default(),
            name_color: Rgba8::rgb(8, 118, 208),
            name_size: 14.0,
            name_stroke: None,
            bubble_color: Rgba8::rgba(255, 255, 255, 235),
            bubble_border: None,
            bubble_radius: 14.0,
            bubble_padding: [12.0, 8.0],
            bubble_frame: None,
            text: TextSpec {
                size: 18.0,
                line_height: 1.45,
                ..TextSpec::default()
            },
            gap: 10.0,
            entrance: HistoryEntrance::SlideUp,
            show_time: false,
            time_color: Rgba8::rgb(127, 149, 174),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TypingSound {
    pub files: Vec<String>,
    pub volume: f32,
    pub pitch_jitter: f32,
    pub every_n: u8,
    pub min_interval_ms: u16,
    pub skip_punct: bool,
}

impl Default for TypingSound {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            volume: 0.6,
            pitch_jitter: 0.05,
            every_n: 1,
            min_interval_ms: 30,
            skip_punct: true,
        }
    }
}

impl TypingSound {
    pub fn rule(&self, enabled: bool) -> TypingSoundRule {
        TypingSoundRule {
            enabled,
            every_n: self.every_n.max(1),
            skip_space: true,
            skip_punct: self.skip_punct,
            min_interval_ms: self.min_interval_ms,
            variants: self.files.len().clamp(1, 255) as u8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Sounds {
    pub typing: TypingSound,
    /// Played when a line starts.
    pub open: Option<String>,
    /// Played on continue.
    pub advance: Option<String>,
    /// Named effects for `[se=name]`.
    pub se: BTreeMap<String, String>,
}

/// Skin defaults for per-character effects. Markup `amp`/`speed` multiply these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectDefaults {
    pub shake_px: f32,
    pub shake_hz: f32,
    pub wave_px: f32,
    pub wave_period_s: f32,
    pub jump_px: f32,
    pub jump_period_s: f32,
    pub rainbow_period_s: f32,
    pub rainbow_saturation: f32,
    pub rainbow_lightness: f32,
    pub screen_shake_px: f32,
}

impl Default for EffectDefaults {
    fn default() -> Self {
        Self {
            shake_px: 1.6,
            shake_hz: 14.0,
            wave_px: 3.0,
            wave_period_s: 1.2,
            jump_px: 6.0,
            jump_period_s: 0.9,
            rainbow_period_s: 2.4,
            rainbow_saturation: 0.75,
            rainbow_lightness: 0.6,
            screen_shake_px: 6.0,
        }
    }
}

/// A complete, resolved skin description.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Skin {
    pub meta: Meta,
    pub chrome: Chrome,
    pub fonts: Fonts,
    pub dialogue: Dialogue,
    pub history: History,
    pub sounds: Sounds,
    pub effects: EffectDefaults,
}

impl Skin {
    pub fn display_name(&self, locale: &str) -> String {
        localized(&self.meta.name, locale, &self.meta.id)
    }

    /// Pacing from the skin's reveal table and the user's speed settings.
    pub fn pacing(&self, speed_scale: f32, punctuation_pauses: bool, typing: bool) -> PacingParams {
        let r = &self.dialogue.reveal;
        PacingParams {
            base_ms: r.base_ms,
            cjk_factor: r.cjk_factor,
            space_ms: r.space_ms,
            punct: r.punct,
            punctuation_pauses,
            speed_scale,
            typing,
        }
    }

    /// Every packaged file the manifest refers to, with the field that names it.
    pub fn referenced_files(&self) -> Vec<(&'static str, &str, FileKind)> {
        let mut out: Vec<(&'static str, &str, FileKind)> = Vec::new();
        for (field, path) in [
            ("meta.preview", &self.meta.preview),
            ("dialogue.background", &self.dialogue.background),
            ("dialogue.portrait.image", &self.dialogue.portrait.image),
            ("dialogue.continue.image", &self.dialogue.continue_.image),
        ] {
            if let Some(p) = path {
                out.push((field, p.as_str(), FileKind::Image));
            }
        }
        for (field, slice) in [
            ("dialogue.box.frame", &self.dialogue.box_.frame),
            ("dialogue.nameplate.frame", &self.dialogue.nameplate.frame),
            ("history.bubble_frame", &self.history.bubble_frame),
        ] {
            if let Some(s) = slice {
                out.push((field, s.image.as_str(), FileKind::NineSlice(s.slice)));
            }
        }
        for path in self.dialogue.portrait.variants.values() {
            out.push(("dialogue.portrait.variants", path.as_str(), FileKind::Image));
        }
        for (field, font) in [
            ("fonts.body.file", &self.fonts.body),
            ("fonts.name.file", &self.fonts.name),
            ("fonts.accent.file", &self.fonts.accent),
        ] {
            if let Some(f) = &font.file {
                out.push((field, f.as_str(), FileKind::Font));
            }
        }
        for f in &self.sounds.typing.files {
            out.push(("sounds.typing.files", f.as_str(), FileKind::Sound));
        }
        for (field, s) in [
            ("sounds.open", &self.sounds.open),
            ("sounds.advance", &self.sounds.advance),
        ] {
            if let Some(s) = s {
                out.push((field, s.as_str(), FileKind::Sound));
            }
        }
        for s in self.sounds.se.values() {
            out.push(("sounds.se", s.as_str(), FileKind::Sound));
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Image,
    NineSlice([u32; 4]),
    Font,
    Sound,
}
