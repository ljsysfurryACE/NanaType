//! The settings window: a sidebar of pages and a scrolling page body.
//! Every control reads the settings signal and writes back through
//! [`Msg::Edit`], so the program stays the only owner of state.

mod controls;
mod pages;

pub use controls::links_pulsing;

use std::sync::Arc;

use nana_ui::icons_tabler as ti;
use nana_ui::runtime::view::{AnyView, IntoView, dynamic, widget};
use nana_ui::runtime::{Activate, ListItem, NodeStyle, ScrollAxes, ScrollView, Stack, Text};
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

fn sidebar(ui: Ui) -> AnyView {
    let items: Vec<AnyView> = Page::ALL
        .into_iter()
        .map(|page| {
            widget(ListItem::new(""))
                .bind(move |item: &mut ListItem| {
                    let label = super::I18N.t_in(ui.locale.get(), page.label_key());
                    if item.label != label {
                        item.label = label;
                    }
                    item.selected = ui.page.get() == page;
                })
                .leading(widget(nana_ui::runtime::IconGlyph::new(page.icon())))
                .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(Msg::SettingsPage(page)))
                .into_any()
        })
        .collect();
    let mut title = NodeStyle::default();
    {
        let l = Arc::make_mut(&mut title.layout);
        l.font_size = Some(18.0);
        l.font_weight = Some(700);
    }
    widget(
        Stack::column(2.0)
            .width(LengthSpec::Px(220.0))
            .padding_xy(10.0, 14.0)
            .surface(SemanticColorRole::Subtle)
            .with_layout(|l| {
                l.height = Some(LengthSpec::Fill);
            }),
    )
    .children((
        widget(Stack::row(8.0).padding_xy(8.0, 6.0)).children((
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
                widget(Stack::column(0.0).padding_xy(28.0, 22.0).max_width(860.0)).children(
                    dynamic(
                        move || (ui.page.get(), ui.locale.get()),
                        move |(page, _)| pages::page(ui, *page),
                    ),
                ),
            ),
        ),
    ))
}
