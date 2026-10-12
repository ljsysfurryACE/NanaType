//! Saves the pictures of chat messages before they are shown.
//!
//! blivechat hands out avatar and emoticon URLs on Bilibili's CDN. Every
//! window loads pictures from local files (the dialogue box and history also
//! feed Spout and snapshots), so each message waits here, on one thread in
//! arrival order, until its avatar and emoticon are saved into the blob
//! folder. Avatars are kept per user (`bili-face-<uid>`), refreshed after a
//! few days; a picture that fails to download is not tried again for a while
//! and the message goes on without it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crossbeam_channel::{Sender, unbounded};
use nanatype_blive::ChatEvent;
use nanatype_model::AssetRef;

use crate::chat::ChatMessage;

/// Avatars older than this are fetched again (people change them).
const AVATAR_TTL: Duration = Duration::from_secs(3 * 24 * 3600);
/// A URL that failed is left alone this long.
const RETRY_AFTER: Duration = Duration::from_secs(10 * 60);
const TIMEOUT: Duration = Duration::from_secs(4);
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const AVATAR_PX: u32 = 128;
const EMOTE_PX: u32 = 160;

/// Feeds events to the saving thread. Dropping it ends the thread.
pub struct ChatAssets {
    tx: Sender<ChatEvent>,
}

impl ChatAssets {
    /// `deliver` receives each message once its pictures are saved.
    pub fn start(blob_dir: PathBuf, deliver: impl Fn(ChatMessage) + Send + 'static) -> Self {
        let (tx, rx) = unbounded::<ChatEvent>();
        thread::Builder::new()
            .name("nanatype-chat-assets".into())
            .spawn(move || {
                let mut saver = Saver::new(blob_dir);
                let mut next = 1u64;
                for event in rx {
                    let (avatar_url, uid, emoticon_url) = match &event {
                        ChatEvent::Text {
                            author, emoticon, ..
                        } => (
                            author.avatar_url.clone(),
                            author.uid.clone(),
                            emoticon.clone(),
                        ),
                        ChatEvent::Gift { author, .. }
                        | ChatEvent::Member { author, .. }
                        | ChatEvent::SuperChat { author, .. } => {
                            (author.avatar_url.clone(), author.uid.clone(), None)
                        }
                        _ => continue,
                    };
                    let avatar = saver.avatar(&uid, &avatar_url);
                    let emoticon = emoticon_url.and_then(|url| saver.emoticon(&url));
                    if let Some(message) = ChatMessage::from_event(event, avatar, emoticon, next) {
                        next += 1;
                        deliver(message);
                    }
                }
            })
            .expect("spawn chat asset thread");
        Self { tx }
    }

    /// Where the blivechat thread hands its events.
    pub fn sender(&self) -> Sender<ChatEvent> {
        self.tx.clone()
    }
}

struct Saver {
    dir: PathBuf,
    agent: ureq::Agent,
    /// Blob name → when it was saved or found fresh this session.
    fresh: HashMap<String, Instant>,
    failed: HashMap<String, Instant>,
}

impl Saver {
    fn new(dir: PathBuf) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(concat!("NanaType/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self {
            dir,
            agent,
            fresh: HashMap::new(),
            failed: HashMap::new(),
        }
    }

    fn avatar(&mut self, uid: &str, url: &str) -> Option<AssetRef> {
        if url.is_empty() {
            return None;
        }
        let name = if uid.is_empty() || !uid.chars().all(|c| c.is_ascii_alphanumeric()) {
            format!("bili-face-u{:016x}", fnv(url))
        } else {
            format!("bili-face-{uid}")
        };
        self.save(&name, url, AVATAR_PX, Some(AVATAR_TTL))
    }

    fn emoticon(&mut self, url: &str) -> Option<AssetRef> {
        let name = format!("bili-emote-{:016x}", fnv(url));
        self.save(&name, url, EMOTE_PX, None)
    }

    /// The blob `name`, downloading `url` into it when it is missing or
    /// older than `ttl`. A stale file is still used when the download fails.
    fn save(&mut self, name: &str, url: &str, px: u32, ttl: Option<Duration>) -> Option<AssetRef> {
        let path = self.dir.join(format!("{name}.png"));
        let asset = AssetRef::Blob(name.to_owned());
        if self.fresh.contains_key(name) {
            return Some(asset);
        }
        let age = file_age(&path);
        if age.is_some_and(|age| ttl.is_none_or(|ttl| age < ttl)) {
            self.fresh.insert(name.to_owned(), Instant::now());
            return Some(asset);
        }
        let failed_recently = self
            .failed
            .get(url)
            .is_some_and(|at| at.elapsed() < RETRY_AFTER);
        if !failed_recently {
            match self.download(url, &path, px) {
                Ok(()) => {
                    self.fresh.insert(name.to_owned(), Instant::now());
                    return Some(asset);
                }
                Err(e) => {
                    crate::debug_log(&format!("chat picture {url}: {e}"));
                    self.failed.insert(url.to_owned(), Instant::now());
                }
            }
        }
        age.map(|_| asset)
    }

    fn download(&self, url: &str, path: &Path, px: u32) -> Result<(), String> {
        let mut response = self.agent.get(url).call().map_err(|e| e.to_string())?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_BYTES)
            .read_to_vec()
            .map_err(|e| e.to_string())?;
        let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        let image = if image.width() > px || image.height() > px {
            image.thumbnail(px, px)
        } else {
            image
        };
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        // Written aside and moved in, so a reader never sees half a file.
        let partial = path.with_extension("part");
        image
            .into_rgba8()
            .save_with_format(&partial, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&partial, path).map_err(|e| e.to_string())
    }
}

fn file_age(path: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(
        SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO),
    )
}

fn fnv(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}
