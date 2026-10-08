//! Parse, inherit and validate a skin. Validation fails closed: a pack with an
//! error is refused as a whole, the way NanaUI refuses a theme that does not
//! compile. Unknown fields only warn so newer packs still load.

use std::path::PathBuf;
use std::sync::Arc;

use toml::{Table, Value};

use crate::files::{FileError, SkinFiles, clean_path};
use crate::manifest::{FileKind, SKIN_SCHEMA, Skin};

pub const MANIFEST: &str = "skin.toml";
pub const MAX_IMAGE_SIDE: usize = 8192;
const MAX_BASE_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinDiagnostic {
    pub severity: Severity,
    /// Dotted manifest path (`dialogue.box.frame`) or a file path.
    pub field: String,
    pub message: String,
}

impl SkinDiagnostic {
    fn error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            field: field.into(),
            message: message.into(),
        }
    }

    fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            field: field.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SkinDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinOrigin {
    Builtin,
    /// An installed or developer folder.
    Directory(PathBuf),
    /// Previewed straight from a `.ntskin` file.
    Archive(PathBuf),
}

/// Files of a skin layered over the files of its bases.
#[derive(Debug)]
pub struct LayeredFiles {
    layers: Vec<Arc<dyn SkinFiles>>,
}

impl SkinFiles for LayeredFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>, FileError> {
        let mut last = FileError::Missing(path.to_owned());
        for layer in &self.layers {
            match layer.read(path) {
                Ok(bytes) => return Ok(bytes),
                Err(FileError::Missing(p)) => last = FileError::Missing(p),
                Err(e) => return Err(e),
            }
        }
        Err(last)
    }

    fn root(&self) -> Option<&std::path::Path> {
        self.layers.first().and_then(|l| l.root())
    }

    fn list(&self) -> Vec<String> {
        let mut all: Vec<String> = self.layers.iter().flat_map(|l| l.list()).collect();
        all.sort();
        all.dedup();
        all
    }
}

#[derive(Debug, Clone)]
pub struct LoadedSkin {
    pub skin: Arc<Skin>,
    pub files: Arc<dyn SkinFiles>,
    pub origin: SkinOrigin,
    pub warnings: Vec<SkinDiagnostic>,
}

/// Look up a base skin's raw manifest table and files by id.
pub trait BaseResolver {
    fn base(&self, id: &str) -> Option<(Table, Arc<dyn SkinFiles>)>;
}

impl BaseResolver for () {
    fn base(&self, _: &str) -> Option<(Table, Arc<dyn SkinFiles>)> {
        None
    }
}

pub fn read_manifest(files: &dyn SkinFiles) -> Result<Table, SkinDiagnostic> {
    let bytes = files
        .read(MANIFEST)
        .map_err(|e| SkinDiagnostic::error(MANIFEST, e.to_string()))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| SkinDiagnostic::error(MANIFEST, "skin.toml is not UTF-8"))?;
    text.parse::<Table>()
        .map_err(|e| SkinDiagnostic::error(MANIFEST, e.to_string().trim().to_owned()))
}

/// `over` wins; tables merge recursively, everything else replaces.
pub fn deep_merge(base: &mut Table, over: &Table) {
    for (key, value) in over {
        match (base.get_mut(key), value) {
            (Some(Value::Table(b)), Value::Table(o)) => deep_merge(b, o),
            _ => {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

fn base_id(table: &Table) -> Option<String> {
    table
        .get("meta")
        .and_then(Value::as_table)
        .and_then(|m| m.get("base"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Report keys present in `raw` but absent from the canonical `known` form.
fn unknown_keys(raw: &Table, known: &Table, prefix: &str, out: &mut Vec<SkinDiagnostic>) {
    for (key, value) in raw {
        let field = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match known.get(key) {
            None => out.push(SkinDiagnostic::warning(field, "unknown field is ignored")),
            Some(Value::Table(k)) => {
                if let Value::Table(r) = value {
                    unknown_keys(r, k, &field, out);
                }
            }
            Some(_) => {}
        }
    }
}

fn version_tuple(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.trim().split(['.', '-', '+']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// Load and validate. `app_version` is this build's version (`CARGO_PKG_VERSION`).
pub fn load(
    files: Arc<dyn SkinFiles>,
    origin: SkinOrigin,
    resolver: &dyn BaseResolver,
    app_version: &str,
) -> Result<LoadedSkin, Vec<SkinDiagnostic>> {
    let raw = read_manifest(&*files).map_err(|d| vec![d])?;
    let mut diagnostics = Vec::new();

    // Inheritance: collect the base chain, then merge from the root down.
    let mut chain = vec![raw.clone()];
    let mut layers = vec![files.clone()];
    let mut next = base_id(&raw);
    let mut seen = Vec::new();
    while let Some(id) = next.take() {
        if seen.contains(&id) || seen.len() >= MAX_BASE_DEPTH {
            return Err(vec![SkinDiagnostic::error(
                "meta.base",
                format!("base chain loops or is deeper than {MAX_BASE_DEPTH}: {id}"),
            )]);
        }
        seen.push(id.clone());
        match resolver.base(&id) {
            Some((table, base_files)) => {
                next = base_id(&table);
                chain.push(table);
                layers.push(base_files);
            }
            None => {
                return Err(vec![SkinDiagnostic::error(
                    "meta.base",
                    format!("base skin `{id}` is not installed"),
                )]);
            }
        }
    }
    let mut merged = Table::new();
    for table in chain.iter().rev() {
        deep_merge(&mut merged, table);
    }
    // The pack's identity is its own, never inherited.
    if let Some(Value::Table(meta)) = merged.get_mut("meta") {
        let own = raw.get("meta").and_then(Value::as_table);
        for key in [
            "id",
            "version",
            "name",
            "description",
            "author",
            "license",
            "preview",
        ] {
            match own.and_then(|m| m.get(key)) {
                Some(v) => {
                    meta.insert(key.into(), v.clone());
                }
                None => {
                    meta.remove(key);
                }
            }
        }
    }

    let skin: Skin = match Value::Table(merged).try_into() {
        Ok(skin) => skin,
        Err(e) => {
            return Err(vec![SkinDiagnostic::error(
                MANIFEST,
                e.to_string().trim().to_owned(),
            )]);
        }
    };
    if let Ok(Value::Table(known)) = Value::try_from(&skin) {
        unknown_keys(&raw, &known, "", &mut diagnostics);
    }

    let files: Arc<dyn SkinFiles> = if layers.len() == 1 {
        files
    } else {
        Arc::new(LayeredFiles { layers })
    };
    validate(&skin, &*files, app_version, &mut diagnostics);
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        diagnostics.sort_by_key(|d| std::cmp::Reverse(d.severity));
        return Err(diagnostics);
    }
    Ok(LoadedSkin {
        skin: Arc::new(skin),
        files,
        origin,
        warnings: diagnostics,
    })
}

fn validate(skin: &Skin, files: &dyn SkinFiles, app_version: &str, out: &mut Vec<SkinDiagnostic>) {
    let meta = &skin.meta;
    if meta.schema > SKIN_SCHEMA {
        out.push(SkinDiagnostic::error(
            "meta.schema",
            format!(
                "schema {} is newer than this app ({SKIN_SCHEMA})",
                meta.schema
            ),
        ));
    }
    let id_ok = !meta.id.is_empty()
        && meta.id.len() <= 64
        && meta
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'));
    if !id_ok {
        out.push(SkinDiagnostic::error(
            "meta.id",
            "id must be 1–64 characters of a-z, 0-9, `.`, `-` or `_`",
        ));
    }
    match (version_tuple(&meta.min_app), version_tuple(app_version)) {
        (Some(need), Some(have)) if need > have => out.push(SkinDiagnostic::error(
            "meta.min_app",
            format!("needs NanaType {} or newer", meta.min_app),
        )),
        (None, _) => out.push(SkinDiagnostic::error("meta.min_app", "not a version")),
        _ => {}
    }
    if version_tuple(&meta.version).is_none() {
        out.push(SkinDiagnostic::warning("meta.version", "not a version"));
    }

    let d = &skin.dialogue;
    let in_range = |v: f32, lo: f32, hi: f32| v.is_finite() && (lo..=hi).contains(&v);
    if !in_range(d.region[0], 160.0, 4096.0) || !in_range(d.region[1], 60.0, 4096.0) {
        out.push(SkinDiagnostic::error(
            "dialogue.region",
            "region must be 160–4096 × 60–4096",
        ));
    }
    if !in_range(d.overscan, 0.0, 256.0) {
        out.push(SkinDiagnostic::error("dialogue.overscan", "must be 0–256"));
    }
    for (field, size) in [
        ("dialogue.text.size", d.text.size),
        ("dialogue.nameplate.size", d.nameplate.size),
        ("history.text.size", skin.history.text.size),
        ("history.name_size", skin.history.name_size),
    ] {
        if !in_range(size, 4.0, 400.0) {
            out.push(SkinDiagnostic::error(field, "font size must be 4–400"));
        }
    }
    if !in_range(d.text.line_height, 0.8, 4.0) {
        out.push(SkinDiagnostic::error(
            "dialogue.text.line_height",
            "must be 0.8–4",
        ));
    }
    if !in_range(d.box_.width_fraction, 0.1, 1.0) {
        out.push(SkinDiagnostic::error(
            "dialogue.box.width_fraction",
            "must be 0.1–1",
        ));
    }
    if d.reveal.base_ms == 0 || d.reveal.base_ms > 2000 {
        out.push(SkinDiagnostic::error(
            "dialogue.reveal.base_ms",
            "must be 1–2000",
        ));
    }

    for (field, path, kind) in skin.referenced_files() {
        let path = match clean_path(path) {
            Ok(p) => p,
            Err(e) => {
                out.push(SkinDiagnostic::error(field, e.to_string()));
                continue;
            }
        };
        let bytes = match files.read(&path) {
            Ok(b) => b,
            Err(e) => {
                out.push(SkinDiagnostic::error(field, e.to_string()));
                continue;
            }
        };
        match kind {
            FileKind::Image | FileKind::NineSlice(_) => match imagesize::blob_size(&bytes) {
                Ok(size) => {
                    if size.width > MAX_IMAGE_SIDE || size.height > MAX_IMAGE_SIDE {
                        out.push(SkinDiagnostic::error(
                            field,
                            format!("`{path}` is larger than {MAX_IMAGE_SIDE}px"),
                        ));
                    }
                    if let FileKind::NineSlice([top, right, bottom, left]) = kind
                        && ((top + bottom) as usize >= size.height
                            || (left + right) as usize >= size.width)
                    {
                        out.push(SkinDiagnostic::error(
                            field,
                            format!(
                                "slice insets do not fit inside the {}×{} image",
                                size.width, size.height
                            ),
                        ));
                    }
                }
                Err(_) => out.push(SkinDiagnostic::error(
                    field,
                    format!("`{path}` is not a PNG, JPEG, WebP or GIF image"),
                )),
            },
            FileKind::Font => {
                if ttf_parser::Face::parse(&bytes, 0).is_err() {
                    out.push(SkinDiagnostic::error(
                        field,
                        format!("`{path}` is not an OpenType/TrueType font"),
                    ));
                }
            }
            FileKind::Sound => {
                let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                if !matches!(ext.as_str(), "wav" | "ogg" | "flac" | "mp3") || bytes.len() < 16 {
                    out.push(SkinDiagnostic::error(
                        field,
                        format!("`{path}` must be a WAV, OGG, FLAC or MP3 file"),
                    ));
                }
            }
        }
    }
}
