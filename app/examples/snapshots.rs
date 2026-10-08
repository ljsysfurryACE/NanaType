//! Render every window's view offscreen to PNG (no desktop windows).
//!
//! `cargo run -p nanatype --example snapshots -- <out-dir> [skin-id|all] [locale]`
//!
//! With `all`, every built-in skin is rendered into `<out-dir>/<skin-id>/`.

use std::path::PathBuf;
use std::sync::Arc;

use nana_ui::runtime::{DocumentId, RuntimeDocument};
use nana_ui_devtools::agent::RuntimeAgentSession;
use nanatype::ui::{Ui, dialogue, history, input, settings};
use nanatype_core::{Cmd, Core};
use nanatype_i18n::Locale;
use nanatype_markup::parse;
use nanatype_settings::Settings;
use nanatype_theme::SkinRegistry;

fn shot(
    name: &str,
    out: &std::path::Path,
    w: u32,
    h: u32,
    theme: Arc<nana_ui::CompiledTheme>,
    mount: impl FnOnce(&mut RuntimeDocument, DocumentId),
) {
    let id = DocumentId::new(1).unwrap();
    let mut doc = RuntimeDocument::new(id);
    let _ = doc.context_mut().set_theme_tokens(theme);
    mount(&mut doc, id);
    let mut session = RuntimeAgentSession::new(doc, w, h).expect("session");
    let path = out.join(format!("{name}.png"));
    match session.screenshot_png(&path) {
        Ok(stats) => println!("{name}: {stats:?}"),
        Err(e) => println!("{name}: {e:?}"),
    }
}

fn main() {
    // Start the session clock now: the lines below are sent at 0 ms, and the
    // render later must see their reveal (and fade-in) as finished.
    let _ = nanatype::clock::now_ms();
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "snapshots".into()));
    let skin_id = args.next().unwrap_or_else(|| "nanatype.halo".into());
    let locale = args
        .next()
        .and_then(|l| Locale::from_tag(&l))
        .unwrap_or(Locale::ZhCn);
    let registry = SkinRegistry::new(None, env!("CARGO_PKG_VERSION"));
    if skin_id == "all" {
        let ids: Vec<String> = registry.entries().iter().map(|e| e.id.clone()).collect();
        for id in ids {
            render(&registry, &out.join(&id), &id, locale);
        }
    } else {
        render(&registry, &out, &skin_id, locale);
    }
}

fn render(registry: &SkinRegistry, out: &std::path::Path, skin_id: &str, locale: Locale) {
    std::fs::create_dir_all(out).unwrap();
    let loaded = registry.load(skin_id).expect("skin");
    let skin = Arc::new(nanatype::skin_theme::prepare(
        &loaded.skin,
        &Default::default(),
        &Default::default(),
    ));
    let mut settings = Settings::default();
    settings.general.speaker_name = "奈奈".into();
    settings.skin.id = skin_id.to_owned();
    settings.history.show_time = true;
    let theme = nanatype::skin_theme::compile(
        &skin,
        skin.chrome.appearance == nanatype_theme::Appearance::Dark,
    );
    let ui = Ui::new(Arc::new(settings.clone()), skin.clone(), locale);
    ui.skins
        .set(Arc::new(nanatype::ui::skin_rows(registry, locale)));
    ui.asset_urls
        .set(Arc::new(nanatype::assets::skin_urls(&*loaded.files)));

    let mut core = Core::new(settings.clone(), skin.clone());
    for line in [
        "大家好！今天也来[b]打字[/b]啦～",
        "[color=#e8505b]红色[/color]、[wave]波浪[/wave]和[ruby=かんじ]漢字[/ruby]注音。[stroke=#e86a92,3][color=#ffffff]描边文字[/color][/stroke][shadow=#0876d0,0,0,8]发光[/shadow]",
    ] {
        core.handle(Cmd::SendMarkup(line.into()), 0);
        core.handle(Cmd::Skip, 1);
    }
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let draft = "[b]粗体[/b]和[shake]抖动[/shake]，[wait=500]然后[pause]继续。[se=coin]";
    let parsed = parse(draft);
    ui.draft.set(Arc::new(parsed.doc.clone()));
    let look = dialogue::Base::editor(&skin).look();
    ui.editor_value.set(nanatype::ui::rich::to_rich(
        &parsed.doc,
        &nanatype::ui::rich::Env {
            look: &look,
            urls: &ui.asset_urls.get_untracked(),
            editor: true,
            locale,
        },
    ));
    let mut src = ui.source.get_untracked();
    src.text = draft.into();
    ui.source.set(src);
    ui.display.set(Arc::new(core.display().clone()));
    ui.history
        .set(Arc::new(core.history().iter().cloned().collect()));
    ui.now.set(100_000);

    let d = &skin.dialogue;
    let (w, h) = (
        (d.region[0] + d.overscan * 2.0) as u32,
        (d.region[1] + d.overscan * 2.0) as u32,
    );
    shot("display", out, w, h, theme.clone(), |doc, id| {
        doc.context_mut()
            .mount_view_root(id, move || {
                dialogue::dialogue_box(ui, dialogue::Source::Display, None)
            })
            .unwrap();
    });
    let ih = (nanatype::windows::TOOLBAR_H
        + d.region[1]
        + d.overscan * 2.0
        + nanatype::windows::STATUS_H) as u32;
    shot("input", out, w, ih, theme.clone(), |doc, id| {
        doc.context_mut()
            .mount_view_root(id, move || input::input_window(ui))
            .unwrap();
    });
    shot(
        "history",
        out,
        skin.history.width as u32,
        skin.history.height as u32,
        theme.clone(),
        |doc, id| {
            doc.context_mut()
                .mount_view_root(id, move || history::history_window(ui))
                .unwrap();
        },
    );
    for page in settings::Page::ALL {
        ui.page.set(page);
        shot(
            &format!("settings-{page:?}"),
            out,
            920,
            660,
            theme.clone(),
            |doc, id| {
                doc.context_mut()
                    .mount_view_root(id, move || settings::settings_window(ui))
                    .unwrap();
            },
        );
    }
}
