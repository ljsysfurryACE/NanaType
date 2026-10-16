use nana_ui::runtime::{DocumentId, RuntimeDocument};
use nana_ui_devtools::agent::RuntimeAgentSession;
use nanatype::ui::{Ui, settings};
use nanatype_i18n::Locale;
use nanatype_settings::Settings;
use std::sync::Arc;

#[test]
fn douyin_page_exposes_native_cookie_actions_without_browser_controls_in_all_locales() {
    let registry = nanatype_theme::SkinRegistry::new(None, "test");
    let skin = registry.load(nanatype_theme::DEFAULT_SKIN).unwrap().skin;
    for locale in Locale::ALL {
        let ui = Ui::new(Arc::new(Settings::default()), skin.clone(), locale);
        ui.page.set(settings::Page::Douyin);
        let id = DocumentId::new(1).unwrap();
        let mut document = RuntimeDocument::new(id);
        document
            .context_mut()
            .set_theme_tokens(nanatype::skin_theme::compile(false))
            .unwrap();
        document
            .context_mut()
            .mount_view_root(id, move || settings::settings_window(ui))
            .unwrap();
        let mut session = RuntimeAgentSession::new(document, 920, 1050).unwrap();
        session.flush().unwrap();
        let nodes = session.accessibility_dump();
        for key in ["dy-import", "dy-logout", "dy-connect"] {
            let label = ui.now_t(key);
            assert_ne!(label, key, "missing translation");
            assert!(
                nodes
                    .iter()
                    .any(|n| n.role == "button" && n.label.as_deref() == Some(label.as_str())),
                "missing action {key} in {locale:?}"
            );
        }
        assert!(
            nodes
                .iter()
                .all(|n| n.label.as_deref().is_none_or(|label| !label
                    .to_lowercase()
                    .contains("browser")
                    && !label.contains("浏览器")
                    && !label.contains("ブラウザー"))),
            "retired browser UI still present in {locale:?}"
        );
    }
}
