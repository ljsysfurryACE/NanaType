//! Platform protected storage. Plaintext credentials never touch a file.
use crate::ApiError;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
pub struct Store {
    #[cfg(windows)]
    path: PathBuf,
}
impl Store {
    pub fn new(directory: &Path) -> Self {
        #[cfg(not(windows))]
        let _ = directory;
        Self {
            #[cfg(windows)]
            path: directory.join("bilibili.dpapi"),
        }
    }
    pub fn load(&self) -> Result<Option<Vec<u8>>, ApiError> {
        #[cfg(windows)]
        {
            match std::fs::read(&self.path) {
                Ok(b) => protect(&b, false).map(Some),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(_) => Err(storage_error()),
            }
        }
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::get_generic_password(
                "dev.sena-nana.nanatype.bilibili",
                "session",
            ) {
                Ok(b) => Ok(Some(b)),
                Err(e) if e.code() == -25300 => Ok(None),
                Err(_) => Err(storage_error()),
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            Ok(None)
        }
    }
    pub fn save(&self, bytes: &[u8]) -> Result<(), ApiError> {
        #[cfg(windows)]
        {
            let encrypted = protect(bytes, true)?;
            std::fs::create_dir_all(self.path.parent().ok_or_else(storage_error)?)
                .map_err(|_| storage_error())?;
            // Atomic replacement contains only DPAPI ciphertext, including the temporary file.
            let temporary = self.path.with_extension("tmp");
            std::fs::write(&temporary, encrypted).map_err(|_| storage_error())?;
            std::fs::rename(temporary, &self.path).map_err(|_| storage_error())
        }
        #[cfg(target_os = "macos")]
        {
            security_framework::passwords::set_generic_password(
                "dev.sena-nana.nanatype.bilibili",
                "session",
                bytes,
            )
            .map_err(|_| storage_error())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = bytes;
            Err(storage_error())
        }
    }
    pub fn clear(&self) -> Result<(), ApiError> {
        #[cfg(windows)]
        {
            let mut failed = false;
            for path in [&self.path, &self.path.with_extension("tmp")] {
                match std::fs::remove_file(path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => failed = true,
                }
            }
            if failed { Err(storage_error()) } else { Ok(()) }
        }
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::delete_generic_password(
                "dev.sena-nana.nanatype.bilibili",
                "session",
            ) {
                Ok(()) => Ok(()),
                Err(e) if e.code() == -25300 => Ok(()),
                Err(_) => Err(storage_error()),
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            Ok(())
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn saved_session_is_encrypted_and_logout_removes_it() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::new(directory.path());
        let fixture = b"SESSDATA=fixture-only; bili_jct=test; DedeUserID=7";
        assert!(store.load().unwrap().is_none());
        store.save(fixture).unwrap();
        let encrypted = std::fs::read(&store.path).unwrap();
        assert!(
            !encrypted
                .windows(fixture.len())
                .any(|bytes| bytes == fixture)
        );
        assert_eq!(store.load().unwrap().unwrap(), fixture);
        store.save(b"replacement fixture").unwrap();
        assert_eq!(store.load().unwrap().unwrap(), b"replacement fixture");
        std::fs::copy(&store.path, store.path.with_extension("tmp")).unwrap();
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
        assert!(!store.path.with_extension("tmp").exists());
    }
    #[test]
    fn unavailable_storage_reports_failure_without_plaintext_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("file");
        std::fs::write(&file, b"existing").unwrap();
        let store = Store::new(&file);
        assert!(store.save(b"fixture").is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"existing");
    }
}
fn storage_error() -> ApiError {
    ApiError::Io("protected account storage unavailable".into())
}
#[cfg(windows)]
fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, ApiError> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into().map_err(|_| storage_error())?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: input is borrowed for this call; DPAPI allocates output, copied then freed once.
    unsafe {
        let ok = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(storage_error());
        }
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        Ok(result)
    }
}
