use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use nanatype_model::Rgba8;

use crate::*;

const APP: &str = "1.0.0";

/// A 4×4 PNG.
fn png(w: u32, h: u32) -> Vec<u8> {
    // Minimal valid PNG header + IHDR is enough for the size probe.
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(b"IHDR");
    out.extend_from_slice(&w.to_be_bytes());
    out.extend_from_slice(&h.to_be_bytes());
    out.extend_from_slice(&[8, 6, 0, 0, 0]);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out
}

fn write_skin(dir: &Path, manifest: &str, files: &[(&str, Vec<u8>)]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(MANIFEST), manifest).unwrap();
    for (path, bytes) in files {
        let p = dir.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
}

fn load_dir(dir: &Path) -> Result<LoadedSkin, Vec<SkinDiagnostic>> {
    let registry = SkinRegistry::new(None, APP);
    struct R<'a>(&'a SkinRegistry);
    impl BaseResolver for R<'_> {
        fn base(&self, id: &str) -> Option<(TomlTable, Arc<dyn SkinFiles>)> {
            let loaded = self.0.load(id).ok()?;
            Some((read_manifest(&*loaded.files).ok()?, loaded.files))
        }
    }
    load(
        Arc::new(DirFiles::new(dir)),
        SkinOrigin::Directory(dir.to_path_buf()),
        &R(&registry),
        APP,
    )
}

fn errors(result: Result<LoadedSkin, Vec<SkinDiagnostic>>) -> Vec<String> {
    match result {
        Ok(_) => Vec::new(),
        Err(d) => d
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.field)
            .collect(),
    }
}

#[test]
fn every_builtin_loads_without_warnings() {
    let registry = SkinRegistry::new(None, APP);
    assert_eq!(registry.entries().len(), 5);
    for entry in registry.entries() {
        assert!(entry.loads(), "{}: {:?}", entry.id, entry.errors);
        assert!(
            entry.warnings.is_empty(),
            "{}: {:?}",
            entry.id,
            entry.warnings
        );
        let skin = registry.load(&entry.id).unwrap();
        assert!(!skin.skin.display_name("ja-JP").is_empty());
    }
    let night = registry.load("nanatype.night").unwrap();
    assert_eq!(night.skin.chrome.appearance, Appearance::Dark);
    assert_eq!(
        night.skin.dialogue.box_.border_color,
        Rgba8::rgb(0x5c, 0xb8, 0xff)
    );
    // Untouched fields keep the defaults.
    assert_eq!(night.skin.dialogue.region, [760.0, 200.0]);
}

#[test]
fn localized_names_fall_back_by_language_then_english() {
    let mut names = Localized::new();
    names.insert("zh-CN".into(), "光环".into());
    names.insert("en-US".into(), "Halo".into());
    assert_eq!(localized(&names, "zh-TW", "x"), "光环");
    assert_eq!(localized(&names, "fr-FR", "x"), "Halo");
    assert_eq!(localized(&Localized::new(), "fr-FR", "x"), "x");
}

#[test]
fn a_user_skin_inherits_from_its_base_and_keeps_its_own_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("sakura");
    write_skin(
        &dir,
        r##"
[meta]
id = "sakura"
base = "nanatype.night"
name = { en-US = "Sakura" }
[dialogue.box]
color = "#ffe0ecf0"
"##,
        &[],
    );
    let skin = load_dir(&dir).unwrap();
    assert_eq!(skin.skin.meta.id, "sakura");
    assert_eq!(
        skin.skin.dialogue.box_.color,
        Rgba8::rgba(0xff, 0xe0, 0xec, 0xf0)
    );
    assert_eq!(
        skin.skin.dialogue.box_.border_color,
        Rgba8::rgb(0x5c, 0xb8, 0xff)
    );
    assert_eq!(skin.skin.display_name("en-US"), "Sakura");
    assert!(skin.warnings.is_empty(), "{:?}", skin.warnings);
}

#[test]
fn unknown_fields_warn_but_load() {
    let tmp = tempfile::tempdir().unwrap();
    write_skin(
        tmp.path(),
        "[meta]\nid = \"x\"\n[dialogue]\nsparkle = true\n",
        &[],
    );
    let skin = load_dir(tmp.path()).unwrap();
    assert_eq!(skin.warnings[0].field, "dialogue.sparkle");
}

#[test]
fn validation_fails_closed() {
    type Case<'a> = (&'a str, &'a str, &'a [(&'a str, Vec<u8>)]);
    let cases: &[Case<'_>] = &[
        ("meta.id", "[meta]\nid = \"Bad Id\"\n", &[]),
        ("meta.schema", "[meta]\nid = \"x\"\nschema = 9\n", &[]),
        (
            "meta.min_app",
            "[meta]\nid = \"x\"\nmin_app = \"9.0.0\"\n",
            &[],
        ),
        ("meta.base", "[meta]\nid = \"x\"\nbase = \"missing\"\n", &[]),
        (
            "dialogue.region",
            "[meta]\nid = \"x\"\n[dialogue]\nregion = [10, 10]\n",
            &[],
        ),
        (
            "dialogue.text.size",
            "[meta]\nid = \"x\"\n[dialogue.text]\nsize = 0\n",
            &[],
        ),
        (
            "dialogue.portrait.image",
            "[meta]\nid = \"x\"\n[dialogue.portrait]\nimage = \"../evil.png\"\n",
            &[],
        ),
        (
            "dialogue.portrait.image",
            "[meta]\nid = \"x\"\n[dialogue.portrait]\nimage = \"missing.png\"\n",
            &[],
        ),
        (
            "dialogue.box.frame",
            "[meta]\nid = \"x\"\n[dialogue.box]\nframe = { image = \"f.png\", slice = [3, 3, 3, 3] }\n",
            &[("f.png", png(4, 4))],
        ),
        (
            "fonts.body.file",
            "[meta]\nid = \"x\"\n[fonts.body]\nfile = \"a.ttf\"\n",
            &[("a.ttf", b"not a font".to_vec())],
        ),
        (
            "sounds.se",
            "[meta]\nid = \"x\"\n[sounds.se]\ncoin = \"coin.txt\"\n",
            &[("coin.txt", vec![0; 64])],
        ),
        (
            "dialogue.continue.image",
            "[meta]\nid = \"x\"\n[dialogue.continue]\nimage = \"c.png\"\n",
            &[("c.png", b"garbage".to_vec())],
        ),
    ];
    for (field, manifest, files) in cases {
        let tmp = tempfile::tempdir().unwrap();
        write_skin(tmp.path(), manifest, files);
        let errs = errors(load_dir(tmp.path()));
        assert!(errs.iter().any(|f| f == field), "{field}: {errs:?}");
    }
}

#[test]
fn malformed_toml_and_bad_colours_are_errors() {
    let tmp = tempfile::tempdir().unwrap();
    write_skin(tmp.path(), "[meta\nid=", &[]);
    assert!(!errors(load_dir(tmp.path())).is_empty());
    write_skin(
        tmp.path(),
        "[meta]\nid = \"x\"\n[dialogue.text]\ncolor = \"red\"\n",
        &[],
    );
    assert!(!errors(load_dir(tmp.path())).is_empty());
}

#[test]
fn a_valid_nine_slice_frame_loads() {
    let tmp = tempfile::tempdir().unwrap();
    write_skin(
        tmp.path(),
        "[meta]\nid = \"x\"\n[dialogue.box]\nframe = { image = \"images/f.png\", slice = [8, 8, 8, 8] }\n",
        &[("images/f.png", png(32, 32))],
    );
    let skin = load_dir(tmp.path()).unwrap();
    assert_eq!(
        skin.skin.dialogue.box_.frame.as_ref().unwrap().slice,
        [8, 8, 8, 8]
    );
}

#[test]
fn clean_path_rejects_escapes() {
    assert_eq!(clean_path("./a\\b.png").unwrap(), "a/b.png");
    for bad in ["../a", "/abs", "C:/x", "a/../../b", ""] {
        assert!(clean_path(bad).is_err(), "{bad}");
    }
}

fn zip_dir(src: &Path, dest: &Path, wrap: Option<&str>) {
    let file = fs::File::create(dest).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for path in DirFiles::new(src).list() {
        let name = match wrap {
            Some(w) => format!("{w}/{path}"),
            None => path.clone(),
        };
        zip.start_file(name, opts).unwrap();
        zip.write_all(&fs::read(src.join(&path)).unwrap()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn install_export_duplicate_and_remove() {
    let tmp = tempfile::tempdir().unwrap();
    let user = tmp.path().join("skins");
    fs::create_dir_all(&user).unwrap();
    let src = tmp.path().join("src");
    write_skin(
        &src,
        "[meta]\nid = \"cats\"\nversion = \"1.2.0\"\n[dialogue.portrait]\nvisible = true\nimage = \"p.png\"\n",
        &[("p.png", png(16, 16))],
    );
    let archive = tmp.path().join("cats.ntskin");
    zip_dir(&src, &archive, Some("cats"));

    let mut registry = SkinRegistry::new(Some(user.clone()), APP);
    assert_eq!(
        registry.install(&archive).unwrap(),
        InstallOutcome::Installed { id: "cats".into() }
    );
    assert!(user.join("cats/p.png").is_file());
    assert!(registry.entry("cats").unwrap().loads());
    assert_eq!(
        registry.install(&archive).unwrap(),
        InstallOutcome::Replaced {
            id: "cats".into(),
            previous: "1.2.0".into()
        }
    );

    let exported = tmp.path().join("out.ntskin");
    registry.export("cats", &exported).unwrap();
    let back = registry.inspect_archive(&exported).unwrap();
    assert_eq!(back.skin.meta.version, "1.2.0");

    let mut over = TomlTable::new();
    let mut text = TomlTable::new();
    text.insert("size".into(), 34.into());
    let mut dialogue = TomlTable::new();
    dialogue.insert("text".into(), text.into());
    over.insert("dialogue".into(), dialogue.into());
    let mut name = Localized::new();
    name.insert("en-US".into(), "Big cats".into());
    registry
        .duplicate("cats", "cats-big", &name, &over)
        .unwrap();
    let big = registry.load("cats-big").unwrap();
    assert_eq!(big.skin.dialogue.text.size, 34.0);
    assert!(big.skin.dialogue.portrait.visible, "inherited from cats");
    assert!(big.files.exists("p.png"), "base files are layered in");

    registry.remove("cats-big").unwrap();
    assert!(registry.entry("cats-big").is_none());
    assert!(registry.remove("nanatype.halo").is_err());
}

#[test]
fn builtin_ids_cannot_be_installed_over() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write_skin(&src, "[meta]\nid = \"nanatype.halo\"\n", &[]);
    let archive = tmp.path().join("x.ntskin");
    zip_dir(&src, &archive, None);
    let user = tmp.path().join("skins");
    fs::create_dir_all(&user).unwrap();
    let mut registry = SkinRegistry::new(Some(user), APP);
    assert!(registry.install(&archive).is_err());
}

#[test]
fn pacing_and_sound_rule_come_from_the_skin() {
    let registry = SkinRegistry::new(None, APP);
    let rpg = registry.load("nanatype.rpg").unwrap();
    let pacing = rpg.skin.pacing(1.0, true, true);
    assert_eq!(pacing.base_ms, 40);
    let rule = rpg.skin.sounds.typing.rule(true);
    assert_eq!(rule.variants, 1);
}
