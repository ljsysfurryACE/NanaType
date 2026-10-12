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
    assert_eq!(registry.entries().len(), 18);
    let mut ids: Vec<&str> = registry.entries().iter().map(|e| e.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 18);
    assert!(registry.load(DEFAULT_SKIN).is_ok());
    for entry in registry.entries() {
        assert!(entry.loads(), "{}: {:?}", entry.id, entry.errors);
        assert!(
            entry.warnings.is_empty(),
            "{}: {:?}",
            entry.id,
            entry.warnings
        );
        let skin = registry.load(&entry.id).unwrap().skin;
        for locale in ["zh-CN", "en-US", "ja-JP"] {
            assert!(skin.meta.name.contains_key(locale), "{} {locale}", entry.id);
            assert!(
                skin.meta.description.contains_key(locale),
                "{} {locale}",
                entry.id
            );
        }
        assert!(!skin.meta.tags.is_empty(), "{}", entry.id);
        for tag in &skin.meta.tags {
            assert!(SKIN_TAGS.contains(&tag.as_str()), "{}: {tag}", entry.id);
        }
    }
    let night = registry.load("nanatype.night").unwrap();
    assert_eq!(night.skin.chrome.appearance, Appearance::Dark);
    assert_eq!(
        night.skin.dialogue.box_.border_color,
        Rgba8::rgb(0x5c, 0xb8, 0xff)
    );
    // Untouched fields keep the defaults.
    assert_eq!(night.skin.dialogue.region, [760.0, 200.0]);
    // A partial `[history.text]` keeps the history defaults, not the dialogue's.
    assert_eq!(night.skin.history.text.size, 18.0);
    let bubble = registry.load("nanatype.bubble").unwrap();
    assert!(bubble.skin.dialogue.tail.is_some());
    let ink = registry.load("nanatype.ink").unwrap();
    assert!(ink.skin.dialogue.decor.len() > 2);
    let echo = registry.load("nanatype.echo_vanilla").unwrap();
    assert_eq!(echo.skin.dialogue.decor.len(), 8);
    assert!(echo.skin.dialogue.continue_.glyph.is_empty());
}

#[test]
fn new_shape_fields_parse_without_warnings() {
    let tmp = tempfile::tempdir().unwrap();
    write_skin(
        tmp.path(),
        r##"
[meta]
id = "x"
tags = ["cute"]
[dialogue.box]
gradient = { kind = "radial", circle = true, stops = [[0, "#fff"], [0.5, "box"], [1.0, "#00000080"]] }
radii = [4, 18, 18, 18]
border_widths = [0, 0, 0, 4]
border_style = "dashed"
shadows = [{ color = "#000", dx = 4, dy = 4 }, { color = "accent", blur = 8, inset = true }]
margin = [0, 0, 24, 0]
[dialogue.tail]
side = "left"
at = 0.5
[[dialogue.decor]]
anchor = "bottom_right"
size = [4, "56%"]
polygon = [[0, 0], [100, 0], [50, 100]]
text = "✦"
text_color = "name_text"
when = "speaker"
[[dialogue.decor]]
inset = [5, 5, 5, 5]
border_color = "border"
border_width = 1
[dialogue.nameplate]
anchor = "top_right"
rotate = -3
border_color = "#000"
border_width = 2
text_shadow = { color = "#000", blur = 4 }
[dialogue.continue]
anchor = "bottom"
offset = [0, -6]
motion = "pulse"
[dialogue.text]
align = "center"
[history]
bubble_radii = [0, 8, 8, 8]
name_plate = { color = "accent", radius = 6 }
avatar = { shape = "circle", ring_color = "#fff", ring_width = 2 }
"##,
        &[],
    );
    let loaded = load_dir(tmp.path()).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let d = &loaded.skin.dialogue;
    let g = d.box_.shape.gradient.as_ref().unwrap();
    assert_eq!(g.kind, GradientKind::Radial);
    assert_eq!(g.stops[1], (0.5, ColorRef::Role(ColorRole::Box)));
    assert_eq!(d.decor[0].size, [Len::Px(4.0), Len::Percent(56.0)]);
    assert_eq!(d.decor[0].text_color, ColorRef::Role(ColorRole::NameText));
    assert_eq!(d.tail.as_ref().unwrap().side, Edge::Left);
    assert_eq!(d.continue_.motion, ContinueMotion::Pulse);
    assert_eq!(d.text.align, TextAlign::Center);
    assert_eq!(loaded.skin.meta.tags, ["cute"]);
}

#[test]
fn a_legacy_manifest_keeps_its_old_meaning() {
    let skin = Skin::default();
    let d = &skin.dialogue;
    assert!(d.tail.is_none());
    assert!(d.decor.is_empty());
    assert!(d.box_.shape.gradient.is_none());
    assert_eq!(d.box_.margin, [0.0; 4]);
    assert_eq!(d.continue_.offset, [-14.0, -8.0]);
    assert_eq!(d.continue_.motion, ContinueMotion::Bob);
    assert_eq!(d.nameplate.anchor, Anchor::TopLeft);
}

#[test]
fn colour_refs_and_lengths_parse() {
    #[derive(serde::Deserialize)]
    struct T {
        c: ColorRef,
        l: Len,
    }
    let t: T = toml::from_str(
        r##"c = "#ff000080"
l = 24"##,
    )
    .unwrap();
    assert_eq!(t.c, ColorRef::Rgba(Rgba8::rgba(255, 0, 0, 128)));
    assert_eq!(t.l, Len::Px(24.0));
    let t: T = toml::from_str(
        r#"c = "name_text"
l = "50%""#,
    )
    .unwrap();
    assert_eq!(t.c, ColorRef::Role(ColorRole::NameText));
    assert_eq!(t.l, Len::Percent(50.0));
    let t: T = toml::from_str(
        r#"c = "box"
l = 24.5"#,
    )
    .unwrap();
    assert_eq!(t.l, Len::Px(24.5));
    for bad in [
        r#"c = "boxy"
l = 1"#,
        r#"c = "box"
l = "50""#,
        r#"c = "box"
l = "abc%""#,
    ] {
        assert!(toml::from_str::<T>(bad).is_err(), "{bad}");
    }
}

#[test]
fn unknown_keys_inside_decor_warn() {
    let tmp = tempfile::tempdir().unwrap();
    write_skin(
        tmp.path(),
        r#"[meta]
id = "x"
[[dialogue.decor]]
text = "a"
[[dialogue.decor]]
sparkle = true
"#,
        &[],
    );
    let skin = load_dir(tmp.path()).unwrap();
    assert_eq!(skin.warnings.len(), 1, "{:?}", skin.warnings);
    assert_eq!(skin.warnings[0].field, "dialogue.decor.1.sparkle");
}

#[test]
fn too_many_decorations_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let mut manifest = String::from("[meta]\nid = \"x\"\n");
    for _ in 0..17 {
        manifest.push_str("[[dialogue.decor]]\nradius = 1\n");
    }
    write_skin(tmp.path(), &manifest, &[]);
    assert!(errors(load_dir(tmp.path())).contains(&"dialogue.decor".to_owned()));
}

#[test]
fn decor_lists_replace_on_inheritance() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("a");
    write_skin(
        &dir,
        r#"[meta]
id = "a"
base = "nanatype.ink"
[[dialogue.decor]]
text = "x"
"#,
        &[],
    );
    let skin = load_dir(&dir).unwrap();
    assert_eq!(skin.skin.dialogue.decor.len(), 1);
    // Tags are inherited; identity is not.
    assert_eq!(skin.skin.meta.tags, ["elegant"]);
    let dir = tmp.path().join("b");
    write_skin(
        &dir,
        r#"[meta]
id = "b"
base = "nanatype.ink"
[dialogue]
decor = []
"#,
        &[],
    );
    assert!(load_dir(&dir).unwrap().skin.dialogue.decor.is_empty());
}

#[test]
fn tails_open_into_the_box() {
    let tail = TailSpec::default();
    let g = tail_geometry(&tail, 4.0);
    // Bottom tail: the fill triangle's base sits above the outer one so it
    // covers the box border, and its tip stays inside the outline.
    assert!(g.inner[0][1] < g.outer[0][1]);
    assert!(g.inner[2][1] < g.outer[2][1]);
    assert!(g.inner[0][0] > g.outer[0][0] && g.inner[1][0] < g.outer[1][0]);
    let flat = tail_geometry(&tail, 0.0);
    assert_eq!(flat.inner, flat.outer);
    // Other sides mirror the same shape.
    let top = tail_geometry(
        &TailSpec {
            side: Edge::Top,
            ..TailSpec::default()
        },
        4.0,
    );
    assert_eq!(top.outer[2][1], 100.0 - g.outer[2][1]);
    assert!(top.rect[1] < 0.0);
    let right = tail_geometry(
        &TailSpec {
            side: Edge::Right,
            ..TailSpec::default()
        },
        4.0,
    );
    assert_eq!(right.outer[2], [g.outer[2][1], g.outer[2][0]]);
    assert_eq!(right.rect[2], g.rect[3]);
}

#[test]
fn placement_follows_the_anchor() {
    let p = place(Anchor::TopLeft, [5.0, 6.0], [Len::Px(10.0), Len::Px(10.0)]);
    assert_eq!(
        (p.left.percent, p.left.px, p.top.percent, p.top.px),
        (0.0, 5.0, 0.0, 6.0)
    );
    let p = place(
        Anchor::BottomRight,
        [-2.0, 0.0],
        [Len::Px(10.0), Len::Px(20.0)],
    );
    assert_eq!((p.left.percent, p.left.px), (100.0, -12.0));
    assert_eq!((p.top.percent, p.top.px), (100.0, -20.0));
    let p = place(
        Anchor::Center,
        [0.0, 0.0],
        [Len::Percent(50.0), Len::Px(4.0)],
    );
    assert_eq!((p.left.percent, p.left.px), (25.0, 0.0));
    assert_eq!((p.top.percent, p.top.px), (50.0, -2.0));
}

#[test]
fn continue_motion_and_sprites() {
    for i in 0..20 {
        let t = i as f32 * 0.07;
        let (dy, o, s) = continue_motion(ContinueMotion::Bob, 3.0, 0.8, t);
        assert!((-3.0..=0.0).contains(&dy) && o == 1.0 && s == 1.0);
        let (_, o, _) = continue_motion(ContinueMotion::Blink, 3.0, 0.8, t);
        assert!(o == 1.0 || o == 0.15);
    }
    assert_eq!(sprite_frame(4, 8.0, 0.0), 0);
    assert_eq!(sprite_frame(4, 8.0, 0.1), 0);
    assert_eq!(sprite_frame(4, 8.0, 0.25), 2);
    assert_eq!(sprite_frame(1, 8.0, 9.0), 0);
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
        (
            "dialogue.box.gradient",
            r##"[meta]
id = "x"
[dialogue.box]
gradient = { stops = [[0, "#fff"]] }
"##,
            &[],
        ),
        (
            "dialogue.box.gradient",
            r##"[meta]
id = "x"
[dialogue.box]
gradient = { stops = [[0.6, "#fff"], [0.2, "#000"]] }
"##,
            &[],
        ),
        (
            "dialogue.box.radius",
            r##"[meta]
id = "x"
[dialogue.box]
radii = [1, 2, -3, 4]
"##,
            &[],
        ),
        (
            "dialogue.box.shadows",
            r##"[meta]
id = "x"
[dialogue.box]
glow = "#fff"
inner_border_color = "#fff"
shadow = { color = "#000" }
shadows = [{ color = "#000" }, { color = "#000" }]
"##,
            &[],
        ),
        (
            "dialogue.box.margin",
            r##"[meta]
id = "x"
[dialogue.box]
margin = [100, 0, 100, 0]
"##,
            &[],
        ),
        (
            "dialogue.decor.0.polygon",
            r##"[meta]
id = "x"
[[dialogue.decor]]
polygon = [[0, 0], [1, 1]]
"##,
            &[],
        ),
        (
            "dialogue.continue.frames",
            r##"[meta]
id = "x"
[dialogue.continue]
frames = 0
"##,
            &[],
        ),
        (
            "dialogue.continue.fps",
            r##"[meta]
id = "x"
[dialogue.continue]
fps = 0
"##,
            &[],
        ),
        (
            "dialogue.tail",
            r##"[meta]
id = "x"
[dialogue.tail]
at = 1.5
"##,
            &[],
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
