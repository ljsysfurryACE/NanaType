//! Only MTProto authorization keys and DC endpoints are persisted. No phone,
//! API hash, password, contacts, messages, or update history is stored.
use grammers_session::{BoxFuture, Session, SessionData, storages::MemorySession, types::*};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Serialize, Deserialize)]
struct Saved {
    home: i32,
    dcs: HashMap<i32, DcOption>,
}

pub struct Store {
    path: PathBuf,
    saved: Mutex<Saved>,
    memory: MemorySession,
}

impl Store {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let data = SessionData::default();
        let saved = match fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice(&protect(&bytes, false)?).map_err(io::Error::other)?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Saved {
                home: data.home_dc,
                dcs: data.dc_options,
            },
            Err(e) => return Err(e),
        };
        Ok(Self {
            path,
            saved: Mutex::new(saved),
            memory: Default::default(),
        })
    }
    fn save(&self, saved: &Saved) -> io::Result<()> {
        let bytes = protect(&serde_json::to_vec(saved)?, true)?;
        fs::create_dir_all(
            self.path
                .parent()
                .ok_or_else(|| io::Error::other("Missing session directory"))?,
        )?;
        let temp = self.path.with_extension("part");
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        let mut file = options.open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temp, &self.path)
    }
    fn data(&self) -> io::Result<std::sync::MutexGuard<'_, Saved>> {
        self.saved
            .lock()
            .map_err(|_| io::Error::other("Telegram session lock poisoned"))
    }
}

impl Session for Store {
    type Error = io::Error;
    fn home_dc_id(&self) -> io::Result<i32> {
        Ok(self.data()?.home)
    }
    fn set_home_dc_id(&self, dc: i32) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            let mut data = self.data()?;
            data.home = dc;
            self.save(&data)
        })
    }
    fn dc_option(&self, id: i32) -> io::Result<Option<DcOption>> {
        Ok(self.data()?.dcs.get(&id).cloned())
    }
    fn set_dc_option(&self, dc: &DcOption) -> BoxFuture<'_, io::Result<()>> {
        let dc = dc.clone();
        Box::pin(async move {
            let mut data = self.data()?;
            data.dcs.insert(dc.id, dc);
            self.save(&data)
        })
    }
    fn peer(&self, id: PeerId) -> BoxFuture<'_, io::Result<Option<PeerInfo>>> {
        Box::pin(async move { self.memory.peer(id).await.map_err(io::Error::other) })
    }
    fn cache_peer(&self, peer: PeerInfo) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move { self.memory.cache_peer(peer).await.map_err(io::Error::other) })
    }
    fn updates_state(&self) -> BoxFuture<'_, io::Result<UpdatesState>> {
        Box::pin(async move { self.memory.updates_state().await.map_err(io::Error::other) })
    }
    fn set_update_state(&self, update: UpdateState) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            self.memory
                .set_update_state(update)
                .await
                .map_err(io::Error::other)
        })
    }
}

#[cfg(windows)]
fn protect(bytes: &[u8], encrypt: bool) -> io::Result<Vec<u8>> {
    use windows::{
        Win32::{
            Foundation::{HLOCAL, LocalFree},
            Security::Cryptography::*,
        },
        core::PCWSTR,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
        .map_err(io::Error::other)?;
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(bytes)
    }
}

#[cfg(not(windows))]
fn protect(bytes: &[u8], _: bool) -> io::Result<Vec<u8>> {
    Ok(bytes.to_vec())
}

pub fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authorization_keys_round_trip_without_login_secrets() {
        let path = std::env::temp_dir().join(format!("nanatype-tg-session-{}", std::process::id()));
        let store = Store::open(path.clone()).unwrap();
        let mut data = store.data().unwrap();
        data.home = 4;
        data.dcs.get_mut(&4).unwrap().auth_key = Some([42; 256]);
        store.save(&data).unwrap();
        drop(data);
        let loaded = Store::open(path.clone()).unwrap();
        assert_eq!(loaded.home_dc_id().unwrap(), 4);
        assert_eq!(
            loaded.dc_option(4).unwrap().unwrap().auth_key,
            Some([42; 256])
        );
        #[cfg(windows)]
        assert!(!String::from_utf8_lossy(&fs::read(&path).unwrap()).contains("auth_key"));
        remove(&path).unwrap();
    }
}
