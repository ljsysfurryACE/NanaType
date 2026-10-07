//! Input toolbar and the settings form. Dialogue pixels are a host texture;
//! the text field under the caret only carries IME.

use std::sync::Arc;

use nana_ui::ButtonKind;
use nana_ui::runtime::view::{EntityRef, IntoView, entity_ref, widget, with_refs};
use nana_ui::runtime::{
    Activate, AppTitleBar, Button, Entity, FlexWrap, FrameworkError, GpuTextureView, LayoutStyle,
    LengthSpec, PositionSpec, RuntimeDocument, ScrollAxes, ScrollView, Stack, Text, TextInput,
};

use crate::program::Message;

pub struct InputUi {
    pub proxy: Entity<TextInput>,
}

pub struct SettingsUi {
    pub name: Entity<TextInput>,
    pub speed: Entity<TextInput>,
    pub fade_base: Entity<TextInput>,
    pub fade_per: Entity<TextInput>,
    pub sticker: Entity<TextInput>,
    pub spout_typewriter: Entity<TextInput>,
    pub spout_history: Entity<TextInput>,
    pub input_w: Entity<TextInput>,
    pub input_h: Entity<TextInput>,
    pub typewriter_w: Entity<TextInput>,
    pub typewriter_h: Entity<TextInput>,
    pub history_w: Entity<TextInput>,
    pub history_h: Entity<TextInput>,
    pub summon: Entity<Text>,
    pub continue_key: Entity<Text>,
    pub status: Entity<Text>,
}

pub fn mount_root(document: &mut RuntimeDocument) -> Result<(), FrameworkError> {
    let id = document.document();
    document
        .context_mut()
        .mount_view_root(id, || widget(Text::new("NanaType")))?;
    Ok(())
}

pub fn mount_input(document: &mut RuntimeDocument) -> Result<InputUi, FrameworkError> {
    let id = document.document();
    let (_, proxy) = document.context_mut().mount_view_root(id, || {
        let proxy = entity_ref::<TextInput>();
        let root = widget(Stack::fill_column(0.0)).with(|column| {
            column.add(toolbar());
            column.add(stage(proxy));
        });
        with_refs(root, proxy)
    })?;
    Ok(InputUi { proxy })
}

pub fn mount_picture(document: &mut RuntimeDocument) -> Result<(), FrameworkError> {
    let id = document.document();
    // A bare texture as the document root does not receive the window size, so
    // the typewriter and history stay fully transparent. Nest it the same way
    // the input stage does.
    document.context_mut().mount_view_root(id, || {
        widget(Stack::fill_column(0.0)).with(|column| {
            column.add(widget(texture_view()));
        })
    })?;
    Ok(())
}

pub fn mount_settings(
    document: &mut RuntimeDocument,
    settings: &dialogue::Settings,
) -> Result<SettingsUi, FrameworkError> {
    let id = document.document();
    let (_, (fields, labels)) = document.context_mut().mount_view_root(id, || {
        let name = entity_ref::<TextInput>();
        let speed = entity_ref::<TextInput>();
        let fade_base = entity_ref::<TextInput>();
        let fade_per = entity_ref::<TextInput>();
        let sticker = entity_ref::<TextInput>();
        let spout_typewriter = entity_ref::<TextInput>();
        let spout_history = entity_ref::<TextInput>();
        let input_w = entity_ref::<TextInput>();
        let input_h = entity_ref::<TextInput>();
        let typewriter_w = entity_ref::<TextInput>();
        let typewriter_h = entity_ref::<TextInput>();
        let history_w = entity_ref::<TextInput>();
        let history_h = entity_ref::<TextInput>();
        let summon = entity_ref::<Text>();
        let continue_key = entity_ref::<Text>();
        let status = entity_ref::<Text>();
        let root = widget(Stack::fill_column(0.0)).with(|column| {
            column.add(widget(AppTitleBar::new("NanaType 设置")));
            column.add(
                widget(ScrollView::new(ScrollAxes::Vertical).with_layout(|layout| {
                    layout.flex_grow = Some(1.0);
                    layout.width = Some(LengthSpec::Fill);
                    layout.height = Some(LengthSpec::Fill);
                }))
                .with(|scroll| {
                    scroll.add(
                        widget(Stack::column(10.0).with_layout(|layout| {
                            layout.padding = Some(LengthSpec::Px(16.0));
                        }))
                        .with(|form| {
                            form.add(widget(Text::new("主题")));
                            form.add(widget(Stack::row(8.0)).with(|row| {
                                row.add(action("光环", Message::Theme(dialogue::ThemeId::Halo)));
                                row.add(action("夜色", Message::Theme(dialogue::ThemeId::Night)));
                                row.add(action(
                                    "羊皮纸",
                                    Message::Theme(dialogue::ThemeId::Parchment),
                                ));
                            }));
                            form.add(widget(Text::new("说话人（留空则不显示名牌）")));
                            form.add(field(&settings.speaker_name, "名字", name));
                            form.add(widget(Text::new("每字毫秒")));
                            form.add(field(&settings.speed_ms.to_string(), "50", speed));
                            form.add(action("切换模拟打字", Message::ToggleTyping));
                            form.add(action("切换弹出", Message::TogglePop));
                            form.add(action("切换逐字同步", Message::TogglePerChar));
                            form.add(action("切换打字音", Message::ToggleSound));
                            form.add(action("切换停留 / 淡出", Message::ToggleFade));
                            form.add(widget(Text::new("淡出基础毫秒 / 每字额外")));
                            form.add(field(&settings.fade_base_ms.to_string(), "1200", fade_base));
                            form.add(field(
                                &settings.fade_per_char_ms.to_string(),
                                "40",
                                fade_per,
                            ));
                            form.add(action("输入常驻", Message::PresenceResident));
                            form.add(action("输入仅热键", Message::PresenceHotkey));
                            form.add(widget(Text::new("召唤热键")).entity_ref(summon));
                            form.add(action("录制召唤热键", Message::CaptureSummon));
                            form.add(widget(Text::new("继续热键")).entity_ref(continue_key));
                            form.add(action("录制继续热键", Message::CaptureContinue));
                            form.add(action("调整布局", Message::LayoutAdjust));
                            form.add(action("显示输入窗", Message::ShowInput));
                            form.add(action("显示打字机", Message::ShowTypewriter));
                            form.add(action("显示历史", Message::ShowHistory));
                            form.add(action("打字机 Spout", Message::SpoutTypewriter));
                            form.add(action("历史 Spout", Message::SpoutHistory));
                            form.add(widget(Text::new("Spout 名称")));
                            form.add(field(
                                &settings.spout_name_typewriter,
                                "NanaType/Typewriter",
                                spout_typewriter,
                            ));
                            form.add(field(
                                &settings.spout_name_history,
                                "NanaType/History",
                                spout_history,
                            ));
                            form.add(widget(Text::new("贴纸目录")));
                            form.add(field(&settings.sticker_dir, "目录", sticker));
                            form.add(action("选择贴纸目录", Message::PickStickers));
                            form.add(widget(Text::new("窗口宽高（输入 / 打字机 / 历史）")));
                            form.add(field(&settings.input_width.to_string(), "760", input_w));
                            form.add(field(&settings.input_height.to_string(), "304", input_h));
                            form.add(field(
                                &settings.typewriter_width.to_string(),
                                "760",
                                typewriter_w,
                            ));
                            form.add(field(
                                &settings.typewriter_height.to_string(),
                                "200",
                                typewriter_h,
                            ));
                            form.add(field(&settings.history_width.to_string(), "420", history_w));
                            form.add(field(
                                &settings.history_height.to_string(),
                                "520",
                                history_h,
                            ));
                            form.add(widget(Text::new("")).entity_ref(status));
                        }),
                    );
                }),
            );
        });
        with_refs(
            root,
            (
                [
                    name,
                    speed,
                    fade_base,
                    fade_per,
                    sticker,
                    spout_typewriter,
                    spout_history,
                    input_w,
                    input_h,
                    typewriter_w,
                    typewriter_h,
                    history_w,
                    history_h,
                ],
                [summon, continue_key, status],
            ),
        )
    })?;
    let [
        name,
        speed,
        fade_base,
        fade_per,
        sticker,
        spout_typewriter,
        spout_history,
        input_w,
        input_h,
        typewriter_w,
        typewriter_h,
        history_w,
        history_h,
    ] = fields;
    let [summon, continue_key, status] = labels;
    Ok(SettingsUi {
        name,
        speed,
        fade_base,
        fade_per,
        sticker,
        spout_typewriter,
        spout_history,
        input_w,
        input_h,
        typewriter_w,
        typewriter_h,
        history_w,
        history_h,
        summon,
        continue_key,
        status,
    })
}

pub fn proxy_layout(x: f32, y: f32) -> Arc<LayoutStyle> {
    let mut layout = LayoutStyle::default();
    layout.position = PositionSpec::Absolute;
    layout.offset_left = Some(LengthSpec::Px(x));
    layout.offset_top = Some(LengthSpec::Px(y));
    layout.width = Some(LengthSpec::Px(180.0));
    layout.height = Some(LengthSpec::Px(32.0));
    layout.opacity = Some(0.0);
    Arc::new(layout)
}

fn texture_view() -> GpuTextureView {
    let mut view = GpuTextureView::new("dialogue").with_pointer_events(true);
    let layout = Arc::make_mut(&mut view.style.layout);
    layout.width = Some(LengthSpec::Fill);
    layout.height = Some(LengthSpec::Fill);
    layout.flex_grow = Some(1.0);
    view
}

fn stage(proxy: EntityRef<TextInput>) -> impl IntoView {
    widget(Stack::fill_column(0.0).with_layout(|layout| {
        layout.position = PositionSpec::Relative;
        layout.flex_grow = Some(1.0);
        layout.flex_shrink = Some(1.0);
        layout.width = Some(LengthSpec::Fill);
        layout.height = Some(LengthSpec::Fill);
    }))
    .with(move |stage| {
        stage.add(widget(texture_view()));
        stage.add(
            widget(
                TextInput::new("")
                    .placeholder("")
                    .layout(proxy_layout(16.0, 16.0)),
            )
            .entity_ref(proxy),
        );
    })
}

fn toolbar() -> impl IntoView {
    widget(Stack::row(6.0).with_layout(|layout| {
        layout.flex_wrap = FlexWrap::Wrap;
        layout.flex_shrink = Some(0.0);
        layout.padding = Some(LengthSpec::Px(8.0));
        layout.width = Some(LengthSpec::Fill);
        layout.height = Some(LengthSpec::Px(crate::paint::TOOLBAR_PX));
    }))
    .with(|row| {
        row.add(action("粗体", Message::Bold));
        row.add(action("斜体", Message::Italic));
        row.add(action("下划线", Message::Underline));
        row.add(action("强调色", Message::ColorAccent));
        row.add(action("正文色", Message::ColorBody));
        row.add(action("放大", Message::Larger));
        row.add(action("缩小", Message::Smaller));
        row.add(action("抖动", Message::Motion(dialogue::Motion::Shake)));
        row.add(action("波浪", Message::Motion(dialogue::Motion::Wave)));
        row.add(action("跳跃", Message::Motion(dialogue::Motion::Jump)));
        row.add(action("彩虹", Message::Motion(dialogue::Motion::Rainbow)));
        row.add(action("静止", Message::Motion(dialogue::Motion::None)));
        row.add(action("等待", Message::Wait));
        row.add(action("变速", Message::SpeedMark));
        row.add(action("瞬显", Message::Instant));
        row.add(action("暂停", Message::Pause));
        row.add(action("图片", Message::PickImage));
        row.add(action("贴纸", Message::NextSticker));
        row.add(action("粘贴", Message::Paste));
        row.add(
            widget(Button::new("发送").kind(ButtonKind::Primary))
                .on_cx(|_, _: &Activate, cx| cx.dispatch_program_all(Message::Submit)),
        );
    })
}

fn action(label: &str, message: Message) -> impl IntoView {
    widget(Button::new(label))
        .on_cx(move |_, _: &Activate, cx| cx.dispatch_program_all(message.clone()))
}

fn field(value: &str, placeholder: &str, slot: EntityRef<TextInput>) -> impl IntoView {
    widget(TextInput::new(value).placeholder(placeholder)).entity_ref(slot)
}
