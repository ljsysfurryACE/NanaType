//! Tray menu. Quitting the process is the only exit.

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub struct Tray {
    _icon: TrayIcon,
    pub show_input: MenuItem,
    pub show_typewriter: MenuItem,
    pub show_history: MenuItem,
    pub settings: MenuItem,
    pub quit: MenuItem,
}

pub fn build() -> Result<Tray, String> {
    let menu = Menu::new();
    let show_input = MenuItem::new("隐藏输入", true, None);
    let show_typewriter = MenuItem::new("隐藏打字机", true, None);
    let show_history = MenuItem::new("隐藏历史", true, None);
    let settings = MenuItem::new("设置", true, None);
    let quit = MenuItem::new("退出", true, None);
    menu.append(&show_input)
        .map_err(|error| error.to_string())?;
    menu.append(&show_typewriter)
        .map_err(|error| error.to_string())?;
    menu.append(&show_history)
        .map_err(|error| error.to_string())?;
    menu.append(&settings).map_err(|error| error.to_string())?;
    menu.append(&quit).map_err(|error| error.to_string())?;
    let icon = tray_icon().map_err(|error| error.to_string())?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("NanaType")
        .with_icon(icon)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(Tray {
        _icon: tray,
        show_input,
        show_typewriter,
        show_history,
        settings,
        quit,
    })
}

pub enum TrayAction {
    ToggleInput,
    ToggleTypewriter,
    ToggleHistory,
    Settings,
    Quit,
}

pub fn poll(tray: &Tray) -> Option<TrayAction> {
    let clicked = matches!(
        TrayIconEvent::receiver().try_recv(),
        Ok(TrayIconEvent::DoubleClick { .. } | TrayIconEvent::Click { .. })
    );
    if clicked {
        return Some(TrayAction::Settings);
    }
    let Ok(event) = MenuEvent::receiver().try_recv() else {
        return None;
    };
    let id = event.id.0;
    if id == tray.show_input.id().0 {
        Some(TrayAction::ToggleInput)
    } else if id == tray.show_typewriter.id().0 {
        Some(TrayAction::ToggleTypewriter)
    } else if id == tray.show_history.id().0 {
        Some(TrayAction::ToggleHistory)
    } else if id == tray.settings.id().0 {
        Some(TrayAction::Settings)
    } else if id == tray.quit.id().0 {
        Some(TrayAction::Quit)
    } else {
        None
    }
}

pub fn sync_labels(tray: &Tray, input: bool, typewriter: bool, history: bool) {
    tray.show_input.set_text(if input {
        "隐藏输入"
    } else {
        "显示输入"
    });
    tray.show_typewriter.set_text(if typewriter {
        "隐藏打字机"
    } else {
        "显示打字机"
    });
    tray.show_history.set_text(if history {
        "隐藏历史"
    } else {
        "显示历史"
    });
}

fn tray_icon() -> Result<Icon, tray_icon::BadIcon> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let d = (dx * dx + dy * dy).sqrt();
            let i = ((y * size + x) * 4) as usize;
            if d <= 14.0 {
                rgba[i] = 8;
                rgba[i + 1] = 118;
                rgba[i + 2] = 208;
                rgba[i + 3] = 255;
            }
            if d <= 6.0 {
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }
    Icon::from_rgba(rgba, size, size)
}
