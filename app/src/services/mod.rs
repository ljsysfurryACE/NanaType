pub mod hotkeys;
pub mod instance;
pub mod persist;
pub mod sounds;
pub mod tray;
pub mod watch;

/// Open a folder (or file) in the system file manager.
pub fn reveal(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

/// Register or remove the "launch at login" entry.
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{
            HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
            RegSetValueExW,
        };
        use windows::core::HSTRING;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let value = format!("\"{}\"", exe.display());
        let mut key = Default::default();
        let path = HSTRING::from(r"Software\Microsoft\Windows\CurrentVersion\Run");
        let name = HSTRING::from("NanaType");
        unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, &path, Some(0), KEY_SET_VALUE, &mut key)
                .ok()
                .map_err(|e| e.to_string())?;
            let result = if enabled {
                let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes = std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2);
                RegSetValueExW(key, &name, Some(0), REG_SZ, Some(bytes))
            } else {
                let r = RegDeleteValueW(key, &name);
                if r.0 == 2 {
                    windows::Win32::Foundation::ERROR_SUCCESS
                } else {
                    r
                }
            };
            let _ = RegCloseKey(key);
            result.ok().map_err(|e| e.to_string())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}
