//! The settings window: a sidebar of pages and a scrolling page body.
//! Every control reads the settings signal and writes back through
//! [`Msg::Edit`], so the program stays the only owner of state.

mod controls;
mod pages;
mod skin_grid;

pub use controls::links_pulsing;
pub use pages::font_licence_panel;

use std::sync::Arc;

use nana_ui::icons_tabler as ti;
use nana_ui::runtime::view::{AnyView, IntoView, dynamic, widget};
use nana_ui::runtime::{Activate, ListItem, NodeStyle, ScrollAxes, ScrollView, Stack, Text};
use nana_ui::theme::{space, type_scale};
use nana_ui_core::{LengthSpec, SemanticColorRole};

use super::Ui;
use crate::msg::Msg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    General,
    Playback,
    Skins,
    Windows,
    History,
    Hotkeys,
    Output,
    Integrations,
    Assets,
    About,
}

impl Page {
    pub const ALL: [Self; 10] = [
        Self::General,
        Self::Playback,
        Self::Skins,
        Self::Windows,
        Self::History,
        Self::Hotkeys,
        Self::Output,
        Self::Integrations,
        Self::Assets,
        Self::About,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Self::General => "settings-general",
            Self::Playback => "settings-playback",
            Self::Skins => "settings-skins",
            Self::Windows => "settings-windows",
            Self::History => "settings-history",
            Self::Hotkeys => "settings-hotkeys",
            Self::Output => "settings-output",
            Self::Integrations => "settings-integrations",
            Self::Assets => "settings-assets",
            Self::About => "settings-about",
        }
    }

    /// The one-line description under the page title.
    pub fn subtitle_key(self) -> &'static str {
        match self {
            Self::General => "settings-general-sub",
            Self::Playback => "settings-playback-sub",
            Self::Skins => "settings-skins-sub",
            Self::Windows => "settings-windows-sub",
            Self::History => "settings-history-sub",
            Self::Hotkeys => "settings-hotkeys-sub",
            Self::Output => "settings-output-sub",
            Self::Integrations => "settings-integrations-sub",
            Self::Assets => "settings-assets-sub",
            Self::About => "settings-about-sub",
        }
    }

    fn icon(self) -> nana_ui::runtime::Icon {
        match self {
            Self::General => ti::ADJUSTMENTS,
            Self::Playback => ti::PLAYER_PLAY,
            Self::Skins => ti::PALETTE,
            Self::Windows => ti::APP_WINDOW,
            Self::History => ti::MESSAGES,
            Self::Hotkeys => ti::KEYBOARD,
            Self::Output => ti::BROADCAST,
            Self::Integrations => ti::PLUG_CONNECTED,
            Self::Assets => ti::STICKER,
            Self::About => ti::INFO_CIRCLE,
        }
    }
}

/// The sidebar's groups: a caption key and its pages. About sits apart, at
/// the foot.
const GROUPS: [(&str, [Page; 3]); 3] = [
    (
        "settings-group-input",
        [Page::General, Page::Playback, Page::Hotkeys],
    ),
    (
        "settings-group-look",
        [Page::Skins, Page::Windows, Page::History],
    ),
    (
        "settings-group-stream",
        [Page::Output, Page::Integrations, Page::Assets],
    ),
];

/// One page's row: a 16px icon (muted, text-coloured when selected) and
/// its name.
fn nav_row(ui: Ui, page: Page) -> AnyView {
    let selected = move || ui.page.get() == page;
    widget(ListItem::new("").size(nana_ui::ControlSize::Small))
        .bind(move |item: &mut ListItem| {
            let label = super::I18N.t_in(ui.locale.get(), page.label_key());
            if item.label != label {
                item.label = label;
            }
            item.selected = selected();
        })
        .leading(
            widget(nana_ui::runtime::IconGlyph::new(page.icon()).size(16.0)).bind(
                move |g: &mut nana_ui::runtime::IconGlyph| {
                    g.style.foreground = Some(if selected() {
                        SemanticColorRole::Text
                    } else {
                        SemanticColorRole::Muted
                    });
                },
            ),
        )
        .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(Msg::SettingsPage(page)))
        .into_any()
}

/// A group's caption.
fn nav_caption(ui: Ui, key: &'static str, first: bool) -> AnyView {
    let mut st = controls::text_style(type_scale::HINT, type_scale::SEMIBOLD);
    {
        let l = Arc::make_mut(&mut st.layout);
        l.margin_top = Some(LengthSpec::Px(if first { space::SM } else { space::XXL }));
        l.margin_bottom = Some(LengthSpec::Px(space::XS));
        l.padding_left = Some(LengthSpec::Px(space::LG));
    }
    widget(
        Text::new(super::I18N.t_in(ui.locale.get_untracked(), key))
            .style(st)
            .color(SemanticColorRole::Muted),
    )
    .bind(move |t: &mut Text| {
        let text = super::I18N.t_in(ui.locale.get(), key);
        if t.value != text {
            t.value = text;
        }
    })
    .into_any()
}

fn sidebar(ui: Ui) -> AnyView {
    let mut items: Vec<AnyView> = Vec::new();
    for (i, (caption, pages)) in GROUPS.into_iter().enumerate() {
        items.push(nav_caption(ui, caption, i == 0));
        items.extend(pages.into_iter().map(|page| nav_row(ui, page)));
    }
    let mut title = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut title.layout);
        l.font_size = Some(type_scale::TITLE);
        l.font_weight = Some(type_scale::BOLD);
    }
    widget(
        Stack::column(space::XXS)
            .width(LengthSpec::Px(196.0))
            .padding_xy(space::LG, space::XXL)
            .surface(SemanticColorRole::Subtle)
            .with_layout(|l| {
                l.height = Some(LengthSpec::Fill);
            }),
    )
    .children((
        widget(Stack::row(space::MD).padding_xy(space::MD, space::SM)).children((
            widget(
                Stack::row(0.0)
                    .width(LengthSpec::Px(28.0))
                    .height(LengthSpec::Px(28.0))
                    .with_layout(|l| {
                        l.paint.background_image =
                            Some(nana_ui_core::BackgroundImage::url_with_fit(
                                format!(
                                    "data:image/png;base64,{}",
                                    crate::ui::settings::controls::ICON_PNG_B64.as_str()
                                ),
                                nana_ui_core::BackgroundImageFit::Contain,
                            ));
                    }),
            ),
            widget(Text::new("NanaType").style(title)),
        )),
        items,
        // About stays at the foot, under whatever the groups leave.
        widget(Stack::column(0.0).grow(1.0)),
        nav_row(ui, Page::About),
    ))
    .into_any()
}

pub fn settings_window(ui: Ui) -> impl IntoView {
    widget(
        Stack::row(0.0)
            .width(LengthSpec::Fill)
            .height(LengthSpec::Fill)
            .surface(SemanticColorRole::Background)
            .align(nana_ui_core::AlignSpec::Stretch),
    )
    .children((
        sidebar(ui),
        widget(Stack::fill_column(0.0)).children(
            widget(ScrollView::new(ScrollAxes::Vertical)).children(
                widget(Stack::column(0.0).max_width(860.0).with_layout(|l| {
                    l.padding_top = Some(LengthSpec::Px(space::PAGE_TIGHT));
                    l.padding_right = Some(LengthSpec::Px(space::PAGE));
                    l.padding_bottom = Some(LengthSpec::Px(space::PAGE));
                    l.padding_left = Some(LengthSpec::Px(space::PAGE));
                }))
                .children(dynamic(
                    move || (ui.page.get(), ui.locale.get()),
                    move |(page, _)| pages::page(ui, *page),
                )),
            ),
        ),
    ))
}
