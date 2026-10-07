mod geom;
mod hotkeys;
mod images;
mod paint;
mod program;
mod sound;
mod surface;
mod theme_ui;
mod tray;
mod ui;

use nana_ui::{WindowDescriptor, WindowShadow, run_runtime};
use program::App;

fn main() -> Result<(), nana_ui::HostedRunError> {
    let mut root = WindowDescriptor::new("NanaType")
        .initial_size(320.0, 180.0)
        .minimum_size(120.0, 80.0)
        .shadow(WindowShadow::None)
        .skip_taskbar(true);
    root.visible = false;
    root.focus_on_show = false;
    run_runtime::<App>(root)
}
