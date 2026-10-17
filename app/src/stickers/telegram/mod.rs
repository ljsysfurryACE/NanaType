//! Official MTProto login/search/download. The resulting folder enters Manager
//! exactly like a local import; this service never publishes renderer resources.
mod session;

use super::{
    cache,
    definition::{self, Definition, Entry},
};
use futures_util::FutureExt;
use grammers_client::{
    Client, SenderPool, SignInError,
    client::{LoginToken, PasswordToken},
    media::Downloadable,
    sender::{ConnectionParams, NoRetries},
    tl,
};
use std::{
    collections::BTreeMap,
    fs,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Offline,
    Code,
    Password,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub name: String,
    pub title: String,
    pub count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub phase: Phase,
    pub busy: bool,
    pub results: Vec<Pack>,
    pub searched: bool,
    pub progress: Option<(usize, usize)>,
    pub error: Option<String>,
}

// Deliberately no Debug: these commands carry short-lived login secrets.
pub enum Command {
    Connect {
        api_id: i32,
        api_hash: String,
        phone: String,
        proxy: String,
    },
    Code(String),
    Password(String),
    Search(String),
    Download(String),
    Logout,
    Cancel,
}

pub enum Event {
    State(State),
    Downloaded(PathBuf),
}

pub struct Service {
    tx: mpsc::UnboundedSender<Command>,
    cancelled: Arc<AtomicBool>,
}

impl Service {
    pub fn new(data: PathBuf, post: impl Fn(Event) + Send + 'static) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        std::thread::Builder::new()
            .name("nanatype-telegram".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                let Ok(runtime) = runtime else {
                    post(Event::State(State {
                        error: Some("Cannot start Telegram worker".into()),
                        ..Default::default()
                    }));
                    return;
                };
                runtime.block_on(async move {
                    let mut worker = Worker {
                        data,
                        connection: None,
                        login: None,
                        password: None,
                        state: Default::default(),
                    };
                    while let Some(command) = rx.recv().await {
                        worker.state.busy = true;
                        worker.state.error = None;
                        post(Event::State(worker.state.clone()));
                        if let Err(error) = worker.handle(command, &flag, &post).await {
                            worker.state.error = Some(error);
                        }
                        worker.state.busy = false;
                        worker.state.progress = None;
                        post(Event::State(worker.state.clone()));
                    }
                });
            })
            .expect("spawn Telegram worker");
        Self { tx, cancelled }
    }
    pub fn send(&self, command: Command) -> Result<(), String> {
        if matches!(command, Command::Cancel) {
            self.cancelled.store(true, Ordering::Relaxed);
            Ok(())
        } else {
            self.cancelled.store(false, Ordering::Relaxed);
            self.tx
                .send(command)
                .map_err(|_| "Telegram worker is unavailable; restart NanaType".into())
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

struct Connection {
    client: Client,
    runner: JoinHandle<()>,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.runner.abort();
    }
}

struct Worker {
    data: PathBuf,
    connection: Option<Connection>,
    login: Option<LoginToken>,
    password: Option<PasswordToken>,
    state: State,
}

async fn request<T, E: std::fmt::Display>(
    future: impl Future<Output = Result<T, E>>,
) -> Result<T, String> {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .map_err(|_| "Telegram request timed out".to_owned())?
        .map_err(|e| e.to_string())
}

pub fn proxy(value: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let url = url::Url::parse(value).map_err(|_| "Invalid SOCKS5 proxy URL")?;
    if url.scheme() != "socks5"
        || url.host_str().is_none()
        || url.port().is_none()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use socks5://host:port or socks5://user:password@host:port".into());
    }
    Ok(Some(value.into()))
}

/// Names are case-insensitive Telegram short names, never filesystem paths.
pub fn pack_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    let name = if value.contains("://") {
        let url = url::Url::parse(value).map_err(|_| "Invalid Telegram sticker link")?;
        if url.scheme() == "tg" && url.host_str() == Some("addstickers") {
            url.query_pairs()
                .find(|(k, _)| k == "set")
                .map(|(_, v)| v.into_owned())
        } else if url.scheme() == "https"
            && matches!(url.host_str(), Some("t.me" | "telegram.me"))
            && url.port().is_none()
            && url.username().is_empty()
        {
            url.path()
                .strip_prefix("/addstickers/")
                .map(|s| s.trim_end_matches('/').into())
        } else {
            None
        }
        .ok_or("Use a t.me/addstickers link or sticker pack name")?
    } else {
        value.into()
    };
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("Invalid Telegram sticker pack name".into());
    }
    Ok(name.to_ascii_lowercase())
}

impl Worker {
    async fn handle(
        &mut self,
        command: Command,
        cancelled: &AtomicBool,
        post: &impl Fn(Event),
    ) -> Result<(), String> {
        match std::panic::AssertUnwindSafe(self.run(command, cancelled, post))
            .catch_unwind()
            .await
        {
            Ok(result) => result,
            Err(_) => {
                // Some login responses, e.g. paid SMS verification, are not
                // handled by the client library yet. Keep retries available.
                self.connection = None;
                self.login = None;
                self.password = None;
                self.state.phase = Phase::Offline;
                Err("Telegram login response is unsupported by this client; try the official Telegram app first".into())
            }
        }
    }

    async fn run(
        &mut self,
        command: Command,
        cancelled: &AtomicBool,
        post: &impl Fn(Event),
    ) -> Result<(), String> {
        match command {
            Command::Connect {
                api_id,
                api_hash,
                phone,
                proxy: value,
            } => {
                let api_hash = api_hash.trim();
                let phone = phone.trim();
                if api_id <= 0 {
                    return Err("Telegram API ID must be a positive number".into());
                }
                let proxy_url = proxy(&value)?;
                self.connection = None;
                self.login = None;
                self.password = None;
                self.state = State {
                    busy: true,
                    ..Default::default()
                };
                let session = Arc::new(
                    session::Store::open(self.data.join("telegram.session"))
                        .map_err(|e| e.to_string())?,
                );
                let SenderPool { runner, handle, .. } = SenderPool::with_configuration(
                    session,
                    api_id,
                    ConnectionParams {
                        proxy_url,
                        device_model: "NanaType".into(),
                        app_version: env!("CARGO_PKG_VERSION").into(),
                        retry_policy: Box::new(NoRetries),
                        ..Default::default()
                    },
                );
                let client = Client::new(handle);
                let runner = tokio::spawn(runner.run());
                self.connection = Some(Connection {
                    client: client.clone(),
                    runner,
                });
                if request(client.is_authorized()).await? {
                    self.state.phase = Phase::Ready;
                } else {
                    if api_hash.len() != 32
                        || !api_hash.bytes().all(|b| b.is_ascii_hexdigit())
                        || !phone.starts_with('+')
                        || phone.len() > 32
                        || phone.len() < 5
                        || !phone[1..].bytes().all(|b| b.is_ascii_digit())
                    {
                        return Err(
                            "Enter API Hash and phone number with country code to log in".into(),
                        );
                    }
                    self.login = Some(request(client.request_login_code(phone, api_hash)).await?);
                    self.state.phase = Phase::Code;
                }
            }
            Command::Code(code) => {
                let connection = self
                    .connection
                    .as_ref()
                    .ok_or("Connect to Telegram first")?;
                let token = self.login.as_ref().ok_or("Request a login code first")?;
                match tokio::time::timeout(
                    Duration::from_secs(30),
                    connection.client.sign_in(token, code.trim()),
                )
                .await
                .map_err(|_| "Telegram request timed out")?
                {
                    Ok(_) => {
                        self.state.phase = Phase::Ready;
                        self.login = None;
                    }
                    Err(SignInError::PasswordRequired(token)) => {
                        self.password = Some(token);
                        self.state.phase = Phase::Password;
                        self.login = None;
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            Command::Password(password) => {
                if self.state.phase != Phase::Password {
                    return Err("No two-step verification in progress".into());
                }
                let connection = self
                    .connection
                    .as_ref()
                    .ok_or("Connect to Telegram first")?;
                let token = match self.password.take() {
                    Some(token) => token,
                    None => PasswordToken::new(
                        request(
                            connection
                                .client
                                .invoke(&tl::functions::account::GetPassword {}),
                        )
                        .await?
                        .into(),
                    ),
                };
                match tokio::time::timeout(
                    Duration::from_secs(30),
                    connection.client.check_password(token, password),
                )
                .await
                .map_err(|_| "Telegram request timed out")?
                {
                    Ok(_) => self.state.phase = Phase::Ready,
                    Err(SignInError::InvalidPassword(token)) => {
                        self.password = Some(token);
                        return Err("Invalid two-step verification password".into());
                    }
                    Err(e) => {
                        self.state.phase = Phase::Offline;
                        return Err(e.to_string());
                    }
                }
            }
            Command::Search(query) => {
                let client = self.authorized()?;
                let query = query.trim();
                if query.is_empty() || query.len() > 256 {
                    return Err(
                        "Enter a search term or sticker pack link (maximum 256 bytes)".into(),
                    );
                }
                let packs = if query.contains("://") {
                    let set = get_set(client, &pack_name(query)?).await?;
                    vec![set.set]
                } else {
                    match request(client.invoke(&tl::functions::messages::SearchStickerSets {
                        exclude_featured: false,
                        q: query.into(),
                        hash: 0,
                    }))
                    .await?
                    {
                        tl::enums::messages::FoundStickerSets::Sets(found) => {
                            found.sets.into_iter().map(|s| s.set()).collect()
                        }
                        _ => Vec::new(),
                    }
                };
                self.state.results = packs
                    .into_iter()
                    .map(|s| {
                        let tl::enums::StickerSet::Set(s) = s;
                        s
                    })
                    .filter(|s| !s.masks && !s.emojis)
                    .take(50)
                    .map(|s| Pack {
                        name: s.short_name,
                        title: s.title,
                        count: s.count.max(0) as usize,
                    })
                    .collect();
                self.state.searched = true;
            }
            Command::Download(name) => {
                let client = self.authorized()?.clone();
                let name = pack_name(&name)?;
                let root = self
                    .data
                    .join("stickers/telegram")
                    .join(format!("tg_{name}"));
                let state = &mut self.state;
                download(&client, &name, &root, cancelled, &|done, total| {
                    let mut state = state.clone();
                    state.progress = Some((done, total));
                    post(Event::State(state));
                })
                .await?;
                post(Event::Downloaded(root));
            }
            Command::Logout => {
                let remote_error = if self.state.phase == Phase::Ready
                    && let Some(connection) = &self.connection
                {
                    request(connection.client.sign_out()).await.err()
                } else {
                    None
                };
                self.connection = None;
                self.login = None;
                self.password = None;
                session::remove(&self.data.join("telegram.session")).map_err(|e| e.to_string())?;
                self.state = Default::default();
                if let Some(error) = remote_error {
                    return Err(format!(
                        "Local session cleared; Telegram sign-out failed: {error}"
                    ));
                }
            }
            Command::Cancel => {}
        }
        Ok(())
    }
    fn authorized(&self) -> Result<&Client, String> {
        if self.state.phase != Phase::Ready {
            return Err("Log in to Telegram first".into());
        }
        self.connection
            .as_ref()
            .map(|c| &c.client)
            .ok_or_else(|| "Connect to Telegram first".into())
    }
}

async fn get_set(client: &Client, name: &str) -> Result<tl::types::messages::StickerSet, String> {
    match request(
        client.invoke(&tl::functions::messages::GetStickerSet {
            stickerset: tl::types::InputStickerSetShortName {
                short_name: name.into(),
            }
            .into(),
            hash: 0,
        }),
    )
    .await?
    {
        tl::enums::messages::StickerSet::Set(set) => Ok(set),
        _ => Err("Telegram returned no sticker pack".into()),
    }
}

struct File(tl::types::Document);
impl Downloadable for File {
    fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        Some(
            tl::types::InputDocumentFileLocation {
                id: self.0.id,
                access_hash: self.0.access_hash,
                file_reference: self.0.file_reference.clone(),
                thumb_size: String::new(),
            }
            .into(),
        )
    }
    fn size(&self) -> Option<usize> {
        usize::try_from(self.0.size).ok()
    }
    fn dc_id(&self) -> Option<i32> {
        Some(self.0.dc_id)
    }
}

async fn download(
    client: &Client,
    name: &str,
    root: &Path,
    cancelled: &AtomicBool,
    progress: &impl Fn(usize, usize),
) -> Result<(), String> {
    if let Ok(definition) = definition::load(root)
        && definition
            .entries
            .iter()
            .all(|e| definition::contained(root, &root.join(&e.path)).is_ok())
    {
        return Ok(());
    }
    let set = get_set(client, name).await?;
    install(name, root, set, cancelled, progress, async |file: &File| {
        let mut iter = client.iter_download(file);
        let mut bytes = Vec::new();
        while let Some(chunk) = request(iter.next()).await? {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            if bytes.len() + chunk.len() > cache::MAX_SOURCE as usize {
                return Err("Telegram sticker exceeds 8 MiB".into());
            }
            bytes.extend(chunk);
        }
        Ok(bytes)
    })
    .await
}

async fn install(
    name: &str,
    root: &Path,
    set: tl::types::messages::StickerSet,
    cancelled: &AtomicBool,
    progress: &impl Fn(usize, usize),
    fetch: impl AsyncFn(&File) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let tl::enums::StickerSet::Set(meta) = set.set;
    if meta.masks
        || meta.emojis
        || set.documents.is_empty()
        || set.documents.len() > definition::MAX_STICKERS
    {
        return Err("Choose a regular Telegram sticker pack".into());
    }
    let parent = root.parent().ok_or("Missing Telegram download directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let stage = parent.join(format!("tg_{name}.part"));
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|e| e.to_string())?;
    }
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = async {
        let mut definition = Definition {
            title: meta.title.clone(),
            author: String::new(),
            license: String::new(),
            repository: Some(format!("https://t.me/addstickers/{name}")),
            entries: Vec::new(),
        };
        let mut total_bytes = 0;
        let total = set.documents.len();
        progress(0, total);
        for (i, document) in set.documents.into_iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            let tl::enums::Document::Document(document) = document else {
                return Err("Telegram sticker file missing".into());
            };
            let ext = match document.mime_type.as_str() {
                "image/webp" => "webp",
                "application/x-tgsticker" => "tgs",
                "video/webm" => "webm",
                _ => {
                    return Err(format!(
                        "Unsupported Telegram sticker format: {}",
                        document.mime_type
                    ));
                }
            };
            if document.size <= 0 || document.size as u64 > cache::MAX_SOURCE {
                return Err("Telegram sticker exceeds 8 MiB".into());
            }
            let title = document
                .attributes
                .iter()
                .find_map(|a| {
                    if let tl::enums::DocumentAttribute::Sticker(s) = a {
                        Some(s.alt.clone())
                    } else {
                        None
                    }
                })
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| document.id.to_string());
            let id = document.id.to_string();
            let file = File(document);
            let bytes = fetch(&file).await?;
            if bytes.len() as i64 != file.0.size {
                return Err("Incomplete Telegram sticker download".into());
            }
            total_bytes += bytes.len();
            if total_bytes > 128 * 1024 * 1024 {
                return Err("Telegram pack exceeds 128 MiB".into());
            }
            let path = PathBuf::from(format!("{id}.{ext}"));
            fs::write(stage.join(&path), bytes).map_err(|e| e.to_string())?;
            definition.entries.push(Entry {
                lib: format!("tg_{name}"),
                name: id,
                title,
                translations: BTreeMap::new(),
                pack_title: meta.title.clone(),
                path,
                height_em: 1.6,
            });
            progress(i + 1, total);
        }
        fs::write(
            stage.join(definition::MANIFEST),
            serde_json::to_vec_pretty(&definition).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            stage.join("SOURCE.txt"),
            format!(
                "{}\n{}\nSticker rights belong to their respective creators.\n",
                meta.title,
                definition.repository.as_deref().unwrap_or_default()
            ),
        )
        .map_err(|e| e.to_string())?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        if root.exists() {
            return Err(
                "Incomplete Telegram pack folder exists; move it aside before retrying".into(),
            );
        }
        fs::rename(&stage, root).map_err(|e| e.to_string())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_uses_authenticated_socks5_and_surfaces_proxy_failure() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = format!("socks5://user:secret@{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut header = [0; 2];
            stream.read_exact(&mut header).unwrap();
            assert_eq!(header[0], 5);
            let mut methods = vec![0; header[1] as usize];
            stream.read_exact(&mut methods).unwrap();
            assert!(methods.contains(&2));
            stream.write_all(&[5, 2]).unwrap();
            stream.read_exact(&mut header).unwrap();
            assert_eq!(header[0], 1);
            let mut user = vec![0; header[1] as usize];
            stream.read_exact(&mut user).unwrap();
            assert_eq!(user, b"user");
            let mut length = [0];
            stream.read_exact(&mut length).unwrap();
            let mut password = vec![0; length[0] as usize];
            stream.read_exact(&mut password).unwrap();
            assert_eq!(password, b"secret");
            stream.write_all(&[1, 0]).unwrap();
            let mut connect = [0; 10];
            stream.read_exact(&mut connect).unwrap();
            assert_eq!(&connect[..4], &[5, 1, 0, 1]);
            assert_ne!(&connect[4..8], &[127, 0, 0, 1]);
            stream.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).unwrap();
        });
        let base = std::env::temp_dir().join(format!("nanatype-tg-proxy-{}", std::process::id()));
        let (tx, rx) = crossbeam_channel::unbounded();
        let service = Service::new(base.clone(), move |event| {
            let _ = tx.send(event);
        });
        service
            .send(Command::Connect {
                api_id: 1,
                api_hash: String::new(),
                phone: String::new(),
                proxy,
            })
            .unwrap();
        loop {
            if let Event::State(state) = rx.recv_timeout(Duration::from_secs(35)).unwrap()
                && !state.busy
            {
                let error = state.error.expect("proxy failure must be visible");
                assert!(!error.contains("secret"), "proxy password leaked");
                break;
            }
        }
        server.join().unwrap();
        drop(service);
        let _ = fs::remove_dir_all(base);
    }

    fn pack(bytes: &[Vec<u8>]) -> tl::types::messages::StickerSet {
        tl::types::messages::StickerSet {
            set: tl::types::StickerSet {
                archived: false,
                official: false,
                masks: false,
                emojis: false,
                text_color: false,
                channel_emoji_status: false,
                creator: false,
                installed_date: None,
                id: 1,
                access_hash: 2,
                title: "Test pack".into(),
                short_name: "TestPack".into(),
                thumbs: None,
                thumb_dc_id: None,
                thumb_version: None,
                thumb_document_id: None,
                count: 3,
                hash: 0,
            }
            .into(),
            packs: Vec::new(),
            keywords: Vec::new(),
            documents: bytes
                .iter()
                .zip(["image/webp", "application/x-tgsticker", "video/webm"])
                .enumerate()
                .map(|(i, (bytes, mime))| {
                    tl::types::Document {
                        id: i as i64 + 1,
                        access_hash: 42,
                        file_reference: vec![1],
                        date: 0,
                        mime_type: mime.into(),
                        size: bytes.len() as i64,
                        thumbs: None,
                        video_thumbs: None,
                        dc_id: 2,
                        attributes: Vec::new(),
                    }
                    .into()
                })
                .collect(),
        }
    }

    #[test]
    fn downloads_are_atomic_cancellable_and_use_the_local_import_pipeline() {
        use super::super::{Manager, Phase as PackPhase, formats::tests::fixture};
        let bytes: Vec<_> = ["webp", "tgs", "webm"].map(fixture).into();
        let base = std::env::temp_dir().join(format!("nanatype-tg-install-{}", std::process::id()));
        let root = base.join("stickers/telegram/tg_testpack");
        let cancelled = AtomicBool::new(true);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let fetch = async |file: &File| Ok(bytes[file.0.id as usize - 1].clone());
            assert!(
                install(
                    "testpack",
                    &root,
                    pack(&bytes),
                    &cancelled,
                    &|_, _| {},
                    fetch
                )
                .await
                .is_err()
            );
            assert!(!root.exists());
            assert!(!root.with_extension("part").exists());
            cancelled.store(false, Ordering::Relaxed);
            assert!(
                install(
                    "testpack",
                    &root,
                    pack(&bytes),
                    &cancelled,
                    &|_, _| {},
                    async |_: &File| Err("Network failure".into())
                )
                .await
                .is_err()
            );
            assert!(!root.exists());
            assert!(!root.with_extension("part").exists());
            assert!(
                install(
                    "testpack",
                    &root,
                    pack(&bytes),
                    &cancelled,
                    &|_, _| {},
                    async |_: &File| Ok(vec![1])
                )
                .await
                .is_err()
            );
            assert!(!root.exists());
            install(
                "testpack",
                &root,
                pack(&bytes),
                &cancelled,
                &|_, _| {},
                fetch,
            )
            .await
            .unwrap();
            // Reusing a complete source folder must not make even one request.
            let session = Arc::new(grammers_session::storages::MemorySession::default());
            let pool = SenderPool::new(session, 1);
            let client = Client::new(pool.handle);
            download(&client, "testpack", &root, &cancelled, &|_, _| {
                panic!("offline reuse downloaded")
            })
            .await
            .unwrap();
        });
        let definition = definition::load(&root).unwrap();
        assert_eq!(definition.entries.len(), 3);
        assert_eq!(definition.entries[0].lib, "tg_testpack");
        assert_eq!(
            definition.repository.as_deref(),
            Some("https://t.me/addstickers/testpack")
        );
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut manager = Manager::new(&base, move |event| {
            tx.send(event).unwrap();
        });
        manager.sync(&[root.display().to_string()]);
        loop {
            let event = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            let completed = matches!(event, super::super::Event::Done { .. });
            manager.accept(event);
            if completed {
                break;
            }
        }
        let root = super::super::folder(&root);
        assert_eq!(
            manager.rows.iter().find(|r| r.root == root).unwrap().phase,
            PackPhase::Ready
        );
        let imported = &manager.loaded[&root];
        assert_eq!(
            imported
                .images
                .iter()
                .map(|c| c.animated())
                .collect::<Vec<_>>(),
            [false, true, true]
        );
        for (entry, cached) in imported.definition.entries.iter().zip(&imported.images) {
            let reused = cache::prepare(
                &root.join(&entry.path),
                &base.join("media/stickers"),
                &cancelled,
            )
            .unwrap();
            assert_eq!(cached.key, reused.key);
        }
        drop(manager);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn links_and_proxy_inputs_are_validated_before_network_access() {
        for input in [
            "Cats",
            "https://t.me/addstickers/Cats",
            "tg://addstickers?set=Cats",
        ] {
            assert_eq!(pack_name(input).unwrap(), "cats");
        }
        for input in [
            "../cats",
            "https://evil.test/addstickers/cats",
            "https://t.me/addstickers/a/b",
            "https://t.me/addemoji/cats",
            "猫",
        ] {
            assert!(pack_name(input).is_err(), "{input}");
        }
        assert_eq!(proxy("").unwrap(), None);
        assert!(proxy("socks5://127.0.0.1:7890").unwrap().is_some());
        assert!(
            proxy("socks5://user:pass@localhost:1080")
                .unwrap()
                .is_some()
        );
        for input in [
            "http://localhost:7890",
            "socks5://localhost",
            "socks5://localhost:1080/path",
        ] {
            assert!(proxy(input).is_err());
        }
    }
}
