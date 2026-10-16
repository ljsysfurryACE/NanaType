//! Exercise the same cached-frame GPU path used by windows and shared outputs.
use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicBool};

use nana_ui::runtime::view::widget;
use nana_ui::runtime::{DocumentId, RichTextView, RuntimeDocument, Stack};
use nana_ui_core::{LengthSpec, RichObjectContent};
use nana_ui_devtools::agent::RuntimeAgentSession;
use nana_ui_devtools::offscreen::{OffscreenSnapshots, Size, write_painter_png};
use nanatype::stickers::{cache, playback::Playback};
use nanatype::ui::{dialogue::Base, rich};
use nanatype::windows::WindowKind;
use nanatype_i18n::Locale;

#[test]
fn settings_show_presets_and_local_folders_in_one_list() {
    use nanatype::stickers::{Manager, PackRow, Phase};
    use nanatype::ui::{Ui, settings};
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/sticker-animation-acceptance");
    std::fs::create_dir_all(&out).unwrap();
    let manager = Manager::new(&out, |_| {});
    for dark in [false, true] {
        let skin = nanatype_theme::SkinRegistry::new(None, "test")
            .load("nanatype.vanilla")
            .unwrap()
            .skin;
        let ui = Ui::new(Arc::new(Default::default()), skin, Locale::ZhCn);
        ui.page.set(settings::Page::Assets);
        let mut rows = manager.rows.clone();
        rows[1].phase = Phase::Processing(7, 20);
        rows.push(PackRow {
            root: out.join("my-emotes"),
            title: "我的表情包".into(),
            author: "".into(),
            license: "".into(),
            count: 12,
            preset: None,
            repository: None,
            phase: Phase::Ready,
        });
        ui.sticker_packs.set(Arc::new(rows));
        let id = DocumentId::new(1).unwrap();
        let mut document = RuntimeDocument::new(id);
        document
            .context_mut()
            .set_theme_tokens(nanatype::skin_theme::compile(dark))
            .unwrap();
        document
            .context_mut()
            .mount_view_root(id, move || settings::settings_window(ui))
            .unwrap();
        let mut session = RuntimeAgentSession::new(document, 1040, 800).unwrap();
        session
            .screenshot_png(out.join(format!(
                "settings-{}.png",
                if dark { "dark" } else { "light" }
            )))
            .unwrap();
        let nodes = session.accessibility_dump();
        for label in [
            "绵羊的像素头像（静态）",
            "绵羊的动态表情包",
            "我的表情包",
            "下载",
        ] {
            assert!(
                nodes.iter().any(|n| n.label.as_deref() == Some(label)),
                "missing {label}"
            );
        }
        for node in nodes.iter().filter(|n| {
            n.role == "button" && matches!(n.label.as_deref(), Some("下载" | "GitHub 仓库"))
        }) {
            assert!(
                node.bounds.x >= 0.0 && node.bounds.x + node.bounds.width <= 1040.0,
                "action must fit: {node:?}"
            );
        }
    }
}

#[test]
fn cached_animation_changes_pixels_without_relayout_and_respects_motion_and_visibility() {
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/sticker-animation-acceptance");
    std::fs::create_dir_all(&out).unwrap();
    let source = out.join("animation.gif");
    let mut writer = image::codecs::gif::GifEncoder::new(std::fs::File::create(&source).unwrap());
    writer
        .set_repeat(image::codecs::gif::Repeat::Finite(1))
        .unwrap();
    for color in [[255, 0, 0, 255], [0, 255, 0, 255]] {
        writer
            .encode_frame(image::Frame::from_parts(
                image::RgbaImage::from_pixel(32, 32, image::Rgba(color)),
                0,
                0,
                image::Delay::from_numer_denom_ms(40, 1),
            ))
            .unwrap();
    }
    drop(writer);
    let cached = cache::prepare(&source, &out.join("cache"), &AtomicBool::new(false)).unwrap();
    assert_eq!(cached.cycles, Some(2));
    cache::publish([cached.clone()].into_iter());
    let skin = nanatype_theme::SkinRegistry::new(None, "test")
        .load("nanatype.vanilla")
        .unwrap()
        .skin;
    let base = Base::editor(&skin);
    let look = base.look();
    let doc = nanatype_markup::parse("[sticker=sprel/laugh h=4]").doc;
    let urls = HashMap::from([("lib:sprel/laugh".into(), cached.url())]);
    let value = rich::to_rich(
        &doc,
        &rich::Env {
            look: &look,
            urls: &urls,
            editor: true,
            locale: Locale::ZhCn,
        },
    );
    assert!(matches!(
        value.objects()[0].1.content,
        RichObjectContent::HostTexture { .. }
    ));
    assert_eq!(rich::from_rich(&value, &look), doc);

    let id = DocumentId::new(1).unwrap();
    let mut document = RuntimeDocument::new(id);
    document
        .context_mut()
        .mount_view_root(id, move || {
            widget(Stack::column(0.0).width(LengthSpec::Fill))
                .children(widget(RichTextView::new(value).style(base.node_style())))
        })
        .unwrap();
    let session = RuntimeAgentSession::new(document, 220, 180).unwrap();
    let scene = session.document().scene().clone();
    let layout = session.document().context().world().layout_generation();
    let mut painter = OffscreenSnapshots::new().unwrap();
    let renderers = painter.default_gpu_renderers();
    let size = Size {
        width: 220,
        height: 180,
    };
    let window = WindowKind::Display.id();
    let mut playback = Playback::default();
    playback.prepare(window, &painter.gpu, 1000, false);
    let registry = playback.registry(window).unwrap();
    painter
        .paint(&scene, size, [0.0; 4], Some(&registry), Some(&renderers))
        .unwrap();
    assert_eq!(playback.next_at(window, 1000, false), Some(1000));
    playback.prepare(window, &painter.gpu, 1000, false);
    let red = painter
        .paint(&scene, size, [0.0; 4], Some(&registry), Some(&renderers))
        .unwrap();
    write_painter_png(&out.join("frame-red.png"), size, &red).unwrap();
    assert!(
        red.chunks_exact(4)
            .filter(|p| p[0] > 240 && p[1] < 10 && p[3] > 240)
            .count()
            > 1000
    );
    assert_eq!(playback.next_at(window, 1000, false), Some(1040));
    playback.prepare(window, &painter.gpu, 1040, false);
    let green = painter
        .paint(&scene, size, [0.0; 4], Some(&registry), Some(&renderers))
        .unwrap();
    write_painter_png(&out.join("frame-green.png"), size, &green).unwrap();
    assert!(
        green
            .chunks_exact(4)
            .filter(|p| p[1] > 240 && p[0] < 10 && p[3] > 240)
            .count()
            > 1000
    );
    assert_ne!(red, green);
    assert_eq!(
        session.document().context().world().layout_generation(),
        layout
    );
    playback.prepare(window, &painter.gpu, 1041, true);
    let reduced = painter
        .paint(&scene, size, [0.0; 4], Some(&registry), Some(&renderers))
        .unwrap();
    assert_eq!(red, reduced);
    assert_eq!(playback.next_at(window, 1041, true), None);
    playback.prepare(window, &painter.gpu, 1160, false);
    painter
        .paint(&scene, size, [0.0; 4], Some(&registry), Some(&renderers))
        .unwrap();
    assert_eq!(playback.next_at(window, 1160, false), None);

    let mut empty = RuntimeDocument::new(id);
    empty
        .context_mut()
        .mount_view_root(id, || widget(Stack::column(0.0)))
        .unwrap();
    let empty = RuntimeAgentSession::new(empty, 220, 180).unwrap();
    painter
        .paint(
            empty.document().scene(),
            size,
            [0.0; 4],
            Some(&registry),
            Some(&renderers),
        )
        .unwrap();
    assert_eq!(
        playback.next_at(window, 1001, false),
        None,
        "a hidden animation does not request frames"
    );
    cache::publish(std::iter::empty::<Arc<cache::CachedImage>>());
}
