//! Place windows on a monitor and remember that monitor's frame.

use dialogue::{Settings, WindowFrame};
use nana_ui_platform::WindowGeometry;
use nana_ui_platform::WindowId;

#[cfg(windows)]
pub fn monitor_key(geometry: WindowGeometry) -> String {
    let (x, y) = geometry.physical_position.unwrap_or((0, 0));
    let (width, height) = geometry.physical_size;
    let cx = x.saturating_add((width / 2) as i32);
    let cy = y.saturating_add((height / 2) as i32);
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint};
    let monitor = unsafe { MonitorFromPoint(POINT { x: cx, y: cy }, MONITOR_DEFAULTTONEAREST) };
    format!("{}", monitor.0 as isize)
}

#[cfg(not(windows))]
pub fn monitor_key(_geometry: WindowGeometry) -> String {
    "primary".into()
}

pub fn remember(settings: &mut Settings, window: &str, geometry: WindowGeometry) {
    let Some((x, y)) = geometry.logical_position else {
        return;
    };
    if geometry.maximized {
        return;
    }
    settings.remember(
        window,
        &monitor_key(geometry),
        WindowFrame {
            x,
            y,
            width: geometry.logical_size.0,
            height: geometry.logical_size.1,
        },
    );
}

/// Bottom-center of the primary work area, in logical pixels.
pub fn input_home(width: f64, height: f64) -> (f64, f64) {
    let (x, y, work_w, work_h) = work_area();
    let left = x + ((work_w - width).max(0.0) / 2.0);
    let top = y + (work_h - height - 28.0).max(0.0);
    (left, top)
}

pub fn default_origin(kind: &str, width: f64, height: f64) -> (f64, f64) {
    let (x, y, work_w, work_h) = work_area();
    match kind {
        "input" => input_home(width, height),
        "typewriter" => (x + 48.0, y + 48.0),
        "history" => (x + (work_w - width - 32.0).max(0.0), y + 48.0),
        _ => (x + 80.0, y + (work_h - height) / 2.0),
    }
}

pub fn saved_or_default(settings: &Settings, kind: &str, width: f64, height: f64) -> (f64, f64) {
    let monitor = current_monitor();
    if let Some(frame) = settings.frame(kind, &monitor) {
        return (f64::from(frame.x), f64::from(frame.y));
    }
    default_origin(kind, width, height)
}

fn current_monitor() -> String {
    monitor_key(WindowGeometry {
        physical_position: Some(cursor_physical()),
        physical_size: (1, 1),
        logical_position: None,
        logical_size: (1.0, 1.0),
        scale_factor: 1.0,
        maximized: false,
    })
}

#[cfg(windows)]
fn cursor_physical() -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_ok() {
        (point.x, point.y)
    } else {
        (0, 0)
    }
}

#[cfg(not(windows))]
fn cursor_physical() -> (i32, i32) {
    (0, 0)
}

#[cfg(windows)]
fn work_area() -> (f64, f64, f64, f64) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{SPI_GETWORKAREA, SystemParametersInfoW};
    let mut rect = RECT::default();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut _),
            Default::default(),
        )
    };
    let scale = dpi_scale();
    if ok.is_ok() {
        (
            f64::from(rect.left) / scale,
            f64::from(rect.top) / scale,
            f64::from(rect.right - rect.left) / scale,
            f64::from(rect.bottom - rect.top) / scale,
        )
    } else {
        (0.0, 0.0, 1280.0, 720.0)
    }
}

#[cfg(windows)]
fn dpi_scale() -> f64 {
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    let dpi = unsafe { GetDpiForSystem() };
    (f64::from(dpi) / 96.0).max(1.0)
}

#[cfg(not(windows))]
fn work_area() -> (f64, f64, f64, f64) {
    (0.0, 0.0, 1280.0, 720.0)
}

pub fn kind_of(
    id: WindowId,
    input: Option<WindowId>,
    typewriter: Option<WindowId>,
    history: Option<WindowId>,
    settings: Option<WindowId>,
) -> Option<&'static str> {
    if Some(id) == input {
        Some("input")
    } else if Some(id) == typewriter {
        Some("typewriter")
    } else if Some(id) == history {
        Some("history")
    } else if Some(id) == settings {
        Some("settings")
    } else {
        None
    }
}
