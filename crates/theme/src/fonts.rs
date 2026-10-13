//! Fonts built into the binary for the built-in skins.
//!
//! A skin folder ships its own font files (`file = "fonts/X.ttf"`); a
//! built-in skin has no folder, so it names an embedded font instead:
//! `file = "builtin:fonts/X.ttf"`, resolved against `crates/theme/fonts/`.
//! `build.rs` embeds whatever is there (with the `builtin-fonts` feature),
//! each font with its licence. A reference to a font this build does not
//! carry is not an error: the role falls back to its `families`, exactly
//! as if the skin had named no file.

use crate::files::clean_path;
use crate::manifest::FontSpec;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/builtin_fonts.rs"));
}

/// What a skin writes before a built-in font's path.
pub const BUILTIN_FONT_PREFIX: &str = "builtin:";

/// One embedded font and the licence that ships with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinFont {
    /// `fonts/<name>.ttf`, relative to `crates/theme/`.
    pub path: &'static str,
    pub bytes: &'static [u8],
    pub license: &'static str,
}

/// A table of embedded fonts; [`BuiltinFonts::embedded`] is this build's.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinFonts {
    fonts: &'static [BuiltinFont],
    known: &'static [&'static str],
}

impl BuiltinFonts {
    /// A table of `fonts`; `known` are the paths a reference may name even
    /// when their bytes are not embedded.
    pub const fn new(fonts: &'static [BuiltinFont], known: &'static [&'static str]) -> Self {
        Self { fonts, known }
    }

    /// The fonts this build embeds: empty without the `builtin-fonts`
    /// feature or when `crates/theme/fonts/` has no fonts.
    pub const fn embedded() -> Self {
        Self::new(generated::EMBEDDED, generated::KNOWN)
    }

    pub fn fonts(&self) -> &'static [BuiltinFont] {
        self.fonts
    }

    /// Every font path in `crates/theme/fonts/`, embedded or not.
    pub fn known(&self) -> &'static [&'static str] {
        self.known
    }

    /// Bytes added to the binary.
    pub fn total_bytes(&self) -> usize {
        self.fonts.iter().map(|f| f.bytes.len()).sum()
    }

    /// The font a `builtin:` reference names, when this table carries it.
    pub fn resolve(&self, reference: &str) -> Option<&'static BuiltinFont> {
        let path = builtin_path(reference)?;
        self.fonts.iter().find(|f| f.path == path)
    }

    /// Whether `reference` names a font in `crates/theme/fonts/`, embedded
    /// or not. An unknown name is most likely a typo.
    pub fn knows(&self, reference: &str) -> bool {
        builtin_path(reference).is_some_and(|p| {
            self.known.contains(&p.as_str()) || self.fonts.iter().any(|f| f.path == p)
        })
    }
}

/// Whether a manifest path names a built-in font rather than a skin file.
pub fn is_builtin_font(reference: &str) -> bool {
    reference.starts_with(BUILTIN_FONT_PREFIX)
}

/// The cleaned path inside `crates/theme/` of a `builtin:` reference.
pub fn builtin_path(reference: &str) -> Option<String> {
    clean_path(reference.strip_prefix(BUILTIN_FONT_PREFIX)?).ok()
}

/// This build's bytes for a `builtin:` reference; `None` when the reference
/// is not one or the font is not embedded, and the role falls back.
pub fn builtin_font(reference: &str) -> Option<&'static [u8]> {
    BuiltinFonts::embedded().resolve(reference).map(|f| f.bytes)
}

/// The family a font file registers under: its typographic family name
/// (name ID 16) when it has one, else its family name (ID 1); the English
/// record before localized ones (a CJK face also names itself in Chinese).
pub fn font_family(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let names = face.names();
    let pick = |id: u16| {
        let mut found: Vec<_> = names
            .into_iter()
            .filter(|n| n.name_id == id && n.is_unicode())
            .collect();
        found.sort_by_key(|n| n.language() != ttf_parser::Language::English_UnitedStates);
        found.into_iter().find_map(|n| n.to_string())
    };
    pick(ttf_parser::name_id::TYPOGRAPHIC_FAMILY).or_else(|| pick(ttf_parser::name_id::FAMILY))
}

impl FontSpec {
    /// Whether the role brings a font file that will actually be used: a
    /// skin file (validated on load), or a built-in font this build embeds.
    pub fn has_usable_file(&self) -> bool {
        match self.file.as_deref() {
            None => false,
            Some(f) if is_builtin_font(f) => builtin_font(f).is_some(),
            Some(_) => true,
        }
    }
}
