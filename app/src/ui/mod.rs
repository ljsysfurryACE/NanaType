//! Views. Every window reads the same [`Ui`] signals, so the input preview,
//! the typewriter and the history list always agree on how a line looks.

pub mod danmaku;
pub mod dialogue;
pub mod history;
pub mod input;
pub mod overflow;
pub mod quote;
pub mod rich;
pub mod settings;
pub mod surface;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use nana_ui::runtime::view::{NodeRef, Signal, node_ref, signal};
use nanatype_core::{DisplayView, HistoryRow};
use nanatype_i18n::{Arg, I18n, Locale};
use nanatype_model::{ChatRef, Document, Entrance, EntranceKind};
use nanatype_settings::Settings;
use nanatype_theme::Skin;

use crate::msg::GlobalAction;
use crate::services::hotkeys::HotkeyState;

pub static I18N: LazyLock<I18n> = LazyLock::new(|| I18n::new(Locale::ZhCn));

/// A notice shown in the input window's status line.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

/// Where an outbound link (VTube Studio, OSC, WebSocket, Spout) stands.
/// `Connected` and `Error` carry the text to show; the other states are
/// labelled in the viewer's locale when drawn.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum LinkState {
    #[default]
    Off,
    Connecting,
    Connected(String),
    NeedsAuth,
    Error(String),
}

/// Connection and device states for the settings pages.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Status {
    pub audio: String,
    pub vts: LinkState,
    pub vts_hotkeys: Vec<(String, String)>,
    pub osc: LinkState,
    pub ws: LinkState,
    /// blivechat: what the panel and the settings say, and the dot's state.
    pub blive: String,
    pub blive_link: danmaku::Link,
    pub spout_display: LinkState,
    pub spout_history: LinkState,
    pub notices: Vec<Notice>,
    pub queued: usize,
    pub audio_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinRow {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The skin's chrome is dark.
    pub dark: bool,
    pub tags: Vec<String>,
    /// The loaded skin, for the picker's preview.
    pub skin: Option<Arc<Skin>>,
    pub author: String,
    pub version: String,
    pub builtin: bool,
    pub error: Option<String>,
    pub folder: Option<std::path::PathBuf>,
    pub accent: [f32; 4],
    pub box_color: [f32; 4],
    pub text_color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct StickerRow {
    pub asset: String,
    pub name: String,
    pub lib: String,
    pub url: String,
}

/// The editor's markup source and selection.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Source {
    pub text: String,
    /// Byte offsets.
    pub selection: (usize, usize),
    /// Bumps when the program (not the user) rewrites the text.
    pub revision: u64,
}

/// Shared reactive state. `Copy`: views capture it freely.
#[derive(Clone, Copy)]
pub struct Ui {
    pub locale: Signal<Locale>,
    pub settings: Signal<Arc<Settings>>,
    pub skin: Signal<Arc<Skin>>,
    pub display: Signal<Arc<DisplayView>>,
    /// Animation clock (ms); advanced every frame while something moves.
    pub now: Signal<u64>,
    pub history: Signal<Arc<Vec<HistoryRow>>>,
    pub draft: Signal<Arc<Document>>,
    pub source: Signal<Source>,
    pub diagnostics: Signal<Arc<Vec<String>>>,
    pub page: Signal<settings::Page>,
    pub status: Signal<Arc<Status>>,
    pub skins: Signal<Arc<Vec<SkinRow>>>,
    pub stickers: Signal<Arc<Vec<StickerRow>>>,
    pub recording: Signal<Option<GlobalAction>>,
    pub hotkeys: Signal<Arc<HashMap<GlobalAction, HotkeyState>>>,
    /// Asset reference → `file://` URL, for documents' atoms.
    pub asset_urls: Signal<Arc<HashMap<String, String>>>,
    /// Portrait variant → `file://` URL for the active skin.
    pub portraits: Signal<Arc<HashMap<String, String>>>,
    pub reduced_motion: Signal<bool>,
    /// Clock (ms) of the input window's mirroring chip pulse; only advanced
    /// while that chip is showing.
    pub pulse_ms: Signal<u64>,
    /// The rich editor's document, as the program last set or received it.
    pub editor_value: Signal<nana_ui_core::RichText>,
    /// Attributes at the editor's selection (toolbar pressed states).
    pub sel_attrs: Signal<nana_ui::runtime::RichSelectionAttrs>,
    /// The editor shows markup source instead of the rich text.
    pub source_mode: Signal<bool>,
    /// The typewriter's text node and the box it is clipped to, read back by
    /// the host to follow long text.
    pub text_node: NodeRef,
    pub text_port: NodeRef,
    /// How far the typewriter's text is scrolled up (logical px).
    pub text_scroll: Signal<f32>,
    /// Live chat, oldest first (capped).
    pub chat: Signal<Arc<Vec<Arc<crate::chat::ChatMessage>>>>,
    /// The chat message the next sent line answers.
    pub reply: Signal<Option<ChatRef>>,
    /// The chat panel's width, so its rows can lay text out to it.
    pub chat_width: Signal<f32>,
}

impl Ui {
    pub fn new(settings: Arc<Settings>, skin: Arc<Skin>, locale: Locale) -> Self {
        let entrance = Entrance {
            kind: EntranceKind::None,
            ms: 0,
        };
        Self {
            locale: signal(locale),
            settings: signal(settings),
            skin: signal(skin),
            display: signal(Arc::new(DisplayView::empty(entrance))),
            now: signal(crate::clock::now_ms()),
            history: signal(Arc::new(Vec::new())),
            draft: signal(Arc::new(Document::new())),
            source: signal(Source::default()),
            diagnostics: signal(Arc::new(Vec::new())),
            page: signal(settings::Page::General),
            status: signal(Arc::new(Status::default())),
            skins: signal(Arc::new(Vec::new())),
            stickers: signal(Arc::new(Vec::new())),
            recording: signal(None),
            hotkeys: signal(Arc::new(HashMap::new())),
            asset_urls: signal(Arc::new(HashMap::new())),
            portraits: signal(Arc::new(HashMap::new())),
            reduced_motion: signal(false),
            pulse_ms: signal(0),
            editor_value: signal(nana_ui_core::RichText::new("")),
            sel_attrs: signal(nana_ui::runtime::RichSelectionAttrs::default()),
            source_mode: signal(false),
            text_node: node_ref(),
            text_port: node_ref(),
            text_scroll: signal(0.0),
            chat: signal(Arc::new(Vec::new())),
            reply: signal(None),
            chat_width: signal(400.0),
        }
    }

    /// A string that follows the language setting.
    pub fn t(self, key: &'static str) -> impl Fn() -> String + Send + 'static {
        let locale = self.locale;
        move || I18N.t_in(locale.get(), key)
    }

    pub fn now_t(self, key: &str) -> String {
        I18N.t_in(self.locale.get_untracked(), key)
    }

    pub fn tr(self, key: &str, args: &[(&str, Arg<'_>)]) -> String {
        I18N.tr_in(self.locale.get(), key, args)
    }
}

/// The skin picker's rows, built-ins first.
pub fn skin_rows(registry: &nanatype_theme::SkinRegistry, locale: Locale) -> Vec<SkinRow> {
    registry
        .entries()
        .iter()
        .map(|e| {
            let mut row = SkinRow {
                id: e.id.clone(),
                name: e.id.clone(),
                description: String::new(),
                dark: false,
                tags: Vec::new(),
                author: e.author.clone(),
                version: e.version.clone(),
                builtin: e.is_builtin(),
                error: e.errors.first().map(|d| d.to_string()),
                folder: match &e.origin {
                    nanatype_theme::SkinOrigin::Directory(p) => Some(p.clone()),
                    _ => None,
                },
                accent: [0.5; 4],
                box_color: [0.5; 4],
                text_color: [0.5; 4],
                skin: e.skin.clone(),
            };
            if let Some(s) = &e.skin {
                let b = &s.dialogue.box_;
                row.name = s.display_name(locale.tag());
                row.description = s.description(locale.tag());
                row.dark = s.chrome.appearance == nanatype_theme::Appearance::Dark;
                row.tags = (s.meta.tags.iter())
                    .filter(|t| nanatype_theme::SKIN_TAGS.contains(&t.as_str()))
                    .cloned()
                    .collect();
                row.accent = c(s.chrome.accent);
                // A gradient box shows its first stop.
                let first = b.shape.gradient.as_ref().and_then(|g| g.stops.first());
                row.box_color = c(first.map_or(b.color, |(_, col)| s.color(*col)));
                row.text_color = c(s.dialogue.text.color);
            }
            row
        })
        .collect()
}

/// Rewrite a bound stack's layout in place.
pub fn restyle(s: &mut nana_ui::runtime::Stack, f: impl FnOnce(&mut nana_ui_core::LayoutStyle)) {
    let taken = std::mem::replace(s, nana_ui::runtime::Stack::row(0.0));
    *s = taken.with_layout(f);
}

/// `[r, g, b, a]` in 0..=1 from a skin colour.
pub fn c(color: nanatype_model::Rgba8) -> [f32; 4] {
    crate::skin_theme::rgba(color)
}
