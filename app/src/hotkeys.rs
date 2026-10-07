//! Global summon and continue keys. Registration lives on one worker thread,
//! the same shape NanaLive uses for background hotkeys.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dialogue::Hotkey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyHit {
    Summon,
    Continue,
}

#[derive(Clone)]
struct Binding {
    summon: Hotkey,
    continue_key: Hotkey,
    suspended: bool,
}

struct Shared {
    binding: Binding,
    revision: u64,
    events: Vec<HotkeyHit>,
    stop: bool,
    status: String,
}

pub struct Hotkeys {
    shared: Arc<Mutex<Shared>>,
    worker: Option<std::thread::JoinHandle<()>>,
    input_hwnd: Arc<AtomicIsize>,
}

impl Hotkeys {
    pub fn new(summon: Hotkey, continue_key: Hotkey) -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared {
                binding: Binding {
                    summon,
                    continue_key,
                    suspended: false,
                },
                revision: 1,
                events: Vec::new(),
                stop: false,
                status: String::new(),
            })),
            worker: None,
            input_hwnd: Arc::new(AtomicIsize::new(0)),
        }
    }

    /// HWND of the input window. The hotkey thread activates it itself:
    /// foreground permission from `WM_HOTKEY` belongs to that thread, not the UI thread.
    pub fn set_input_hwnd(&self, hwnd: isize) {
        self.input_hwnd.store(hwnd, Ordering::Release);
    }

    pub fn poll(
        &mut self,
        summon: &Hotkey,
        continue_key: &Hotkey,
        suspended: bool,
    ) -> Vec<HotkeyHit> {
        let mut shared = self
            .shared
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if shared.binding.summon != *summon
            || shared.binding.continue_key != *continue_key
            || shared.binding.suspended != suspended
        {
            shared.binding.summon = summon.clone();
            shared.binding.continue_key = continue_key.clone();
            shared.binding.suspended = suspended;
            shared.revision = shared.revision.wrapping_add(1);
            shared.events.clear();
        }
        let events = std::mem::take(&mut shared.events);
        drop(shared);
        if self.worker.is_none() {
            let shared = Arc::clone(&self.shared);
            let input_hwnd = Arc::clone(&self.input_hwnd);
            match std::thread::Builder::new()
                .name("nanatype-hotkeys".into())
                .spawn(move || run(shared, input_hwnd))
            {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    if let Ok(mut shared) = self.shared.lock() {
                        shared.status = format!("无法启动全局热键：{error}");
                    }
                }
            }
        }
        events
    }

    pub fn status(&self) -> String {
        self.shared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .status
            .clone()
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.stop = true;
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(windows))]
fn run(shared: Arc<Mutex<Shared>>, _input_hwnd: Arc<AtomicIsize>) {
    if let Ok(mut shared) = shared.lock() {
        shared.status = "全局热键仅在 Windows 上注册".into();
    }
}

#[cfg(windows)]
fn run(shared: Arc<Mutex<Shared>>, input_hwnd: Arc<AtomicIsize>) {
    use windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
    use windows::Win32::UI::WindowsAndMessaging::{
        MSG, PM_NOREMOVE, PM_REMOVE, PeekMessageW, WM_HOTKEY,
    };

    // RegisterHotKey delivers to this thread only after it owns a queue.
    let mut warmup = MSG::default();
    let _ = unsafe { PeekMessageW(&mut warmup, None, 0, 0, PM_NOREMOVE) };

    let mut applied_revision = 0u64;
    let mut ids: Vec<(i32, HotkeyHit)> = Vec::new();
    loop {
        let (binding, revision, stop) = {
            let state = shared.lock().unwrap_or_else(|error| error.into_inner());
            (state.binding.clone(), state.revision, state.stop)
        };
        if stop {
            break;
        }
        // `revision` bumps whenever the binding changes, so it is the only
        // generation the worker needs to compare.
        if applied_revision != revision {
            for (id, _) in ids.drain(..) {
                unsafe {
                    let _ = UnregisterHotKey(None, id);
                }
            }
            let mut failures = Vec::new();
            if !binding.suspended {
                register(
                    &binding.summon,
                    1,
                    HotkeyHit::Summon,
                    &mut ids,
                    &mut failures,
                );
                register(
                    &binding.continue_key,
                    2,
                    HotkeyHit::Continue,
                    &mut ids,
                    &mut failures,
                );
            }
            if let Ok(mut state) = shared.lock() {
                state.status = if failures.is_empty() {
                    String::new()
                } else {
                    format!("热键未注册：{}", failures.join("，"))
                };
            }
            applied_revision = revision;
        }
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }.as_bool()
        {
            let hit = ids
                .iter()
                .find(|(id, _)| *id == message.wParam.0 as i32)
                .map(|(_, hit)| *hit);
            if let Some(hit) = hit
                && let Ok(state) = shared.lock()
                && state.revision == revision
                && !state.binding.suspended
            {
                drop(state);
                if hit == HotkeyHit::Summon {
                    let hwnd = locate_input(input_hwnd.load(Ordering::Acquire));
                    let _ = activate_input(hwnd);
                }
                if let Ok(mut state) = shared.lock()
                    && state.revision == revision
                    && !state.binding.suspended
                {
                    state.events.push(hit);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    for (id, _) in ids {
        unsafe {
            let _ = UnregisterHotKey(None, id);
        }
    }
}

#[cfg(windows)]
fn register(
    hotkey: &Hotkey,
    id: i32,
    hit: HotkeyHit,
    ids: &mut Vec<(i32, HotkeyHit)>,
    failures: &mut Vec<String>,
) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    };
    let Some(vk) = virtual_key(&hotkey.key) else {
        failures.push(hotkey.label());
        return;
    };
    let mut modifiers = MOD_NOREPEAT;
    if hotkey.ctrl {
        modifiers |= MOD_CONTROL;
    }
    if hotkey.shift {
        modifiers |= MOD_SHIFT;
    }
    if hotkey.alt {
        modifiers |= MOD_ALT;
    }
    if hotkey.win {
        modifiers |= MOD_WIN;
    }
    let registered = unsafe { RegisterHotKey(None, id, modifiers, vk) }.is_ok()
        || unsafe { RegisterHotKey(None, id, modifiers_without_norepeat(modifiers), vk) }.is_ok();
    if registered {
        ids.push((id, hit));
    } else {
        failures.push(hotkey.label());
    }
}

#[cfg(windows)]
fn modifiers_without_norepeat(
    modifiers: windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS,
) -> windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS {
    use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, MOD_NOREPEAT};
    HOT_KEY_MODIFIERS(modifiers.0 & !MOD_NOREPEAT.0)
}

#[cfg(windows)]
fn locate_input(cached: isize) -> isize {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindow};
    if cached != 0 {
        let hwnd = HWND(cached as *mut core::ffi::c_void);
        if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return cached;
        }
    }
    let name = windows::core::HSTRING::from("输入");
    unsafe { FindWindowW(None, &name) }
        .ok()
        .map(|hwnd| hwnd.0 as isize)
        .filter(|hwnd| *hwnd != 0)
        .unwrap_or(0)
}

#[cfg(windows)]
fn activate_input(hwnd: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_MENU, keybd_event,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow,
        GetWindowThreadProcessId, IsIconic, SW_RESTORE, SW_SHOW, SWP_NOMOVE, SWP_NOSIZE,
        SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, ShowWindow,
    };
    if hwnd == 0 {
        return false;
    }
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    unsafe {
        // WM_HOTKEY gives this thread permission to take the foreground.
        // Do that before any Alt injection: a menu key can lock the foreground again.
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        let _ = BringWindowToTop(hwnd);
        if SetForegroundWindow(hwnd).as_bool() {
            return true;
        }
        keybd_event(VK_MENU.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let ours = GetCurrentThreadId();
        let attach_foreground = foreground_thread != 0
            && foreground_thread != ours
            && AttachThreadInput(ours, foreground_thread, true).as_bool();
        let attach_target = target_thread != 0
            && target_thread != ours
            && target_thread != foreground_thread
            && AttachThreadInput(ours, target_thread, true).as_bool();
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let raised = SetForegroundWindow(hwnd).as_bool();
        if attach_target {
            let _ = AttachThreadInput(ours, target_thread, false);
        }
        if attach_foreground {
            let _ = AttachThreadInput(ours, foreground_thread, false);
        }
        raised
    }
}

#[cfg(windows)]
fn virtual_key(code: &str) -> Option<u32> {
    if let Some(letter) = code.strip_prefix("Key").filter(|value| value.len() == 1) {
        let byte = letter.as_bytes()[0];
        return byte.is_ascii_uppercase().then_some(u32::from(byte));
    }
    if let Some(digit) = code.strip_prefix("Digit").filter(|value| value.len() == 1) {
        let byte = digit.as_bytes()[0];
        return byte.is_ascii_digit().then_some(u32::from(byte));
    }
    if let Some(number) = code
        .strip_prefix('F')
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| (1..=24).contains(value))
    {
        return Some(0x70 + number - 1);
    }
    Some(match code {
        "Space" => 0x20,
        "Tab" => 0x09,
        "Escape" => 0x1B,
        "Enter" => 0x0D,
        "Minus" => 0xBD,
        "Equal" => 0xBB,
        _ => return None,
    })
}
