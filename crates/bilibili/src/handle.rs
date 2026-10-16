use crate::{
    ApiError, ChatEvent, Client, Profile, QrStatus, Room,
    credentials::Store,
    live_ws::{HEARTBEAT_INTERVAL, LiveEvent, LiveSocket},
    sending::{self, Delivery, DeliveryState},
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use nanatype_model::LineId;
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BilibiliConfig {
    pub enabled: bool,
    pub room: Option<u64>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AccountState {
    #[default]
    Restoring,
    SignedOut,
    Qr {
        url: String,
        status: QrStatus,
    },
    Ready(Profile),
    Expired,
    Failed(String),
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConnectionState {
    #[default]
    Off,
    NeedsLogin,
    Connecting,
    Connected(Room),
    Error(String),
}
#[derive(Debug, Clone)]
pub enum Event {
    Account(AccountState),
    Connection(ConnectionState),
    Chat { room: u64, event: ChatEvent },
    Delivery(Delivery),
    Warning(String),
}
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub epoch: u64,
    pub event: Event,
}
enum Command {
    Configure(u64, BilibiliConfig),
    Login(u64),
    CancelLogin(u64),
    Refresh(u64),
    Logout(u64),
    Send {
        permit: Permit,
        epoch: u64,
        send_epoch: u64,
        line: LineId,
        text: String,
    },
    Retry(LineId),
    SkipFailed(LineId),
}
pub struct BilibiliHandle {
    capacity: Arc<AtomicUsize>,
    tx: Sender<Command>,
    epoch: Arc<AtomicU64>,
    send_epoch: Arc<AtomicU64>,
    sending: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    config: Option<BilibiliConfig>,
}
impl BilibiliHandle {
    pub fn new(directory: PathBuf, deliver: impl Fn(SessionEvent) + Send + Sync + 'static) -> Self {
        let (tx, rx) = unbounded();
        let epoch = Arc::new(AtomicU64::new(0));
        let send_epoch = Arc::new(AtomicU64::new(0));
        let sending = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let worker = Worker {
            rx,
            epoch: epoch.clone(),
            send_epoch: send_epoch.clone(),
            sending: sending.clone(),
            alive: alive.clone(),
            deliver: Arc::new(deliver),
            store: Store::new(&directory),
            client: Client::in_memory(),
            profile: None,
            config: BilibiliConfig::default(),
            room: None,
            queue: VecDeque::new(),
            seen: HashSet::new(),
            login: None,
            next_poll: Instant::now(),
            next_send: Instant::now(),
            loaded: false,
        };
        thread::Builder::new()
            .name("nanatype-bilibili".into())
            .spawn(move || worker.run())
            .expect("start Bilibili service");
        Self {
            capacity: Arc::new(AtomicUsize::new(0)),
            tx,
            epoch,
            send_epoch,
            sending,
            alive,
            config: None,
        }
    }
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }
    fn advance(&self) -> u64 {
        self.set_sending(false);
        self.epoch.fetch_add(1, Ordering::AcqRel) + 1
    }
    pub fn set(&mut self, config: BilibiliConfig) {
        if self.config.as_ref() == Some(&config) {
            return;
        }
        self.config = Some(config.clone());
        let epoch = self.advance();
        let _ = self.tx.send(Command::Configure(epoch, config));
    }
    pub fn login(&self) {
        let e = self.advance();
        let _ = self.tx.send(Command::Login(e));
    }
    pub fn cancel_login(&self) {
        let e = self.advance();
        let _ = self.tx.send(Command::CancelLogin(e));
    }
    pub fn refresh(&self) {
        let e = self.advance();
        let _ = self.tx.send(Command::Refresh(e));
    }
    pub fn logout(&self) {
        let e = self.advance();
        let _ = self.tx.send(Command::Logout(e));
    }
    pub fn set_sending(&self, value: bool) {
        if !value {
            self.send_epoch.fetch_add(1, Ordering::AcqRel);
        }
        self.sending.store(value, Ordering::Release);
    }
    pub fn send(&self, line: LineId, text: String) -> bool {
        let Some(permit) = Permit::acquire(self.capacity.clone()) else {
            return false;
        };
        self.tx
            .send(Command::Send {
                permit,
                epoch: self.epoch(),
                send_epoch: self.send_epoch.load(Ordering::Acquire),
                line,
                text,
            })
            .is_ok()
    }
    pub fn retry(&self, line: LineId) {
        let _ = self.tx.send(Command::Retry(line));
    }
    pub fn skip_failed(&self, line: LineId) {
        let _ = self.tx.send(Command::SkipFailed(line));
    }
}
impl Drop for BilibiliHandle {
    fn drop(&mut self) {
        self.set_sending(false);
        self.alive.store(false, Ordering::Release);
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }
}
type Deliver = Arc<dyn Fn(SessionEvent) + Send + Sync>;
struct Pending {
    _permit: Permit,
    delivery: Delivery,
    send_epoch: u64,
    epoch: u64,
}
/// A slot is reserved before posting to the worker, including while HTTP is busy.
struct Permit(Arc<AtomicUsize>);
impl Permit {
    fn acquire(capacity: Arc<AtomicUsize>) -> Option<Self> {
        let mut current = capacity.load(Ordering::Acquire);
        loop {
            if current >= 100 {
                return None;
            }
            match capacity.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(value) => current = value,
            }
        }
        Some(Self(capacity))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Worker {
    rx: Receiver<Command>,
    epoch: Arc<AtomicU64>,
    send_epoch: Arc<AtomicU64>,
    sending: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    deliver: Deliver,
    store: Store,
    client: Client,
    profile: Option<Profile>,
    config: BilibiliConfig,
    room: Option<Room>,
    queue: VecDeque<Pending>,
    seen: HashSet<LineId>,
    login: Option<(u64, crate::QrLogin, Instant)>,
    next_poll: Instant,
    next_send: Instant,
    loaded: bool,
}
impl Worker {
    fn emit(&self, epoch: u64, event: Event) {
        (self.deliver)(SessionEvent { epoch, event });
    }
    fn current(&self, e: u64) -> bool {
        self.alive.load(Ordering::Acquire) && self.epoch.load(Ordering::Acquire) == e
    }
    fn run(mut self) {
        while self.alive.load(Ordering::Acquire) {
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(c) => self.command(c),
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                Err(_) => {}
            }
            self.poll_login();
            self.cancel_stale();
            self.send_next();
        }
    }
    fn command(&mut self, c: Command) {
        match c {
            Command::Configure(e, c) => {
                if !self.current(e) {
                    return;
                }
                self.config = c;
                self.login = None;
                if !self.loaded {
                    self.loaded = true;
                    match self.store.load() {
                        Ok(Some(bytes)) => {
                            if let Err(error) = self.client.restore(&bytes) {
                                self.emit(
                                    e,
                                    Event::Account(AccountState::Failed(error.to_string())),
                                );
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.emit(e, Event::Account(AccountState::Failed(error.to_string())));
                            return;
                        }
                    }
                }
                self.refresh(e);
            }
            Command::Login(e) => {
                if !self.current(e) {
                    return;
                }
                self.room = None;
                self.profile = None;
                self.login = None;
                self.emit(e, Event::Connection(ConnectionState::NeedsLogin));
                self.client = Client::in_memory();
                match self.client.qr_generate() {
                    Ok(q) if self.current(e) => {
                        self.emit(
                            e,
                            Event::Account(AccountState::Qr {
                                url: q.url.clone(),
                                status: QrStatus::Waiting,
                            }),
                        );
                        self.login = Some((e, q, Instant::now()));
                        self.next_poll = Instant::now() + Duration::from_secs(2);
                    }
                    Err(error) => {
                        self.emit(e, Event::Account(AccountState::Failed(error.to_string())))
                    }
                    _ => {}
                }
            }
            Command::CancelLogin(e) => {
                if !self.current(e) {
                    return;
                }
                self.login = None;
                self.client.clear();
                self.profile = None;
                self.room = None;
                self.emit(e, Event::Account(AccountState::SignedOut));
                self.emit(e, Event::Connection(ConnectionState::NeedsLogin));
            }
            Command::Refresh(e) => {
                if self.current(e) {
                    self.login = None;
                    self.refresh(e);
                }
            }
            Command::Logout(e) => {
                if !self.current(e) {
                    return;
                }
                self.login = None;
                self.room = None;
                self.profile = None;
                self.client.clear();
                if let Err(error) = self.store.clear() {
                    self.emit(e, Event::Account(AccountState::Failed(error.to_string())));
                } else {
                    self.emit(e, Event::Account(AccountState::SignedOut));
                }
                self.emit(e, Event::Connection(ConnectionState::NeedsLogin));
            }
            Command::Send {
                permit,
                epoch,
                send_epoch,
                line,
                text,
            } => {
                if !self.current(epoch)
                    || !self.sending.load(Ordering::Acquire)
                    || send_epoch != self.send_epoch.load(Ordering::Acquire)
                    || !self.seen.insert(line)
                {
                    return;
                }
                let (Some(room), Some(profile)) = (&self.room, &self.profile) else {
                    return;
                };
                let result = sending::split(&text, room.max_chars);
                let mut delivery = Delivery {
                    line,
                    room: room.id,
                    uid: profile.uid,
                    chunks: vec![text],
                    completed: 0,
                    state: DeliveryState::Queued,
                    detail: String::new(),
                    blocked: false,
                    receipts: Vec::new(),
                };
                match result {
                    Ok(chunks) if self.queue.len() < 100 => {
                        delivery.chunks = chunks;
                        self.emit(epoch, Event::Delivery(delivery.clone()));
                        self.queue.push_back(Pending {
                            _permit: permit,
                            delivery,
                            send_epoch,
                            epoch,
                        });
                    }
                    Ok(_) => {
                        delivery.state = DeliveryState::Failed;
                        delivery.detail = "send queue is full".into();
                        self.emit(epoch, Event::Delivery(delivery));
                    }
                    Err(error) => {
                        delivery.state = DeliveryState::Failed;
                        delivery.detail = error.to_string();
                        self.emit(epoch, Event::Delivery(delivery));
                    }
                }
            }
            Command::Retry(line) => {
                if let Some(p) = self.queue.front_mut()
                    && p.delivery.line == line
                    && p.delivery.state == DeliveryState::Failed
                {
                    p.delivery.state = DeliveryState::Queued;
                    p.delivery.blocked = false;
                }
            }
            Command::SkipFailed(line) => {
                if self
                    .queue
                    .front()
                    .is_some_and(|p| p.delivery.blocked && p.delivery.line == line)
                {
                    self.cancel_front();
                }
            }
        }
    }
    fn refresh(&mut self, e: u64) {
        self.room = None;
        if !self.client.is_logged_in() {
            self.profile = None;
            self.emit(e, Event::Account(AccountState::SignedOut));
            self.emit(e, Event::Connection(ConnectionState::NeedsLogin));
            return;
        }
        self.emit(e, Event::Account(AccountState::Restoring));
        match self.client.profile() {
            Ok(p) if self.current(e) => {
                self.emit(e, Event::Account(AccountState::Ready(p.clone())));
                self.profile = Some(p);
                self.connect(e);
            }
            Err(error) if self.current(e) => {
                self.profile = None;
                let state = if is_auth_error(&error) {
                    self.client.clear();
                    AccountState::Expired
                } else {
                    AccountState::Failed(error.to_string())
                };
                self.emit(e, Event::Account(state));
                self.emit(e, Event::Connection(ConnectionState::NeedsLogin));
            }
            _ => {}
        }
    }
    fn connect(&mut self, e: u64) {
        if !self.config.enabled {
            self.emit(e, Event::Connection(ConnectionState::Off));
            return;
        }
        let id = self
            .config
            .room
            .or_else(|| self.profile.as_ref().and_then(|p| p.room_id));
        let Some(id) = id else {
            self.emit(e, Event::Connection(ConnectionState::Off));
            return;
        };
        self.emit(e, Event::Connection(ConnectionState::Connecting));
        match self.client.room(id) {
            Ok(room) if self.current(e) => {
                self.room = Some(room.clone());
                let client = self.client.snapshot();
                let epoch = self.epoch.clone();
                let alive = self.alive.clone();
                let deliver = self.deliver.clone();
                thread::Builder::new()
                    .name("nanatype-bilibili-chat".into())
                    .spawn(move || receive(client, room, e, epoch, alive, deliver))
                    .expect("start chat receiver");
            }
            Err(error) => self.emit(
                e,
                Event::Connection(ConnectionState::Error(error.to_string())),
            ),
            _ => {}
        }
    }
    fn poll_login(&mut self) {
        self.poll_login_using(|client, key| client.qr_poll(key));
    }
    fn poll_login_using(
        &mut self,
        poll: impl FnOnce(&Client, &str) -> Result<crate::QrPoll, ApiError>,
    ) {
        if Instant::now() < self.next_poll {
            return;
        }
        let Some((e, q, started)) = self.login.clone() else {
            return;
        };
        if !self.current(e) {
            self.login = None;
            return;
        }
        self.next_poll = Instant::now() + Duration::from_secs(2);
        if started.elapsed() > Duration::from_secs(180) {
            self.login = None;
            self.emit(
                e,
                Event::Account(AccountState::Qr {
                    url: q.url,
                    status: QrStatus::Expired,
                }),
            );
            return;
        }
        // Polling may complete after cancel, room change or a replacement QR request.
        let client = self.client.snapshot();
        match poll(&client, &q.qrcode_key) {
            Ok(poll) if self.current(e) => {
                if poll.status == QrStatus::Confirmed {
                    self.login = None;
                    self.client = client;
                    if let Err(error) = self.store.save(&self.client.credentials()) {
                        self.emit(e, Event::Warning(error.to_string()));
                    }
                    self.refresh(e);
                } else {
                    if poll.status == QrStatus::Expired {
                        self.login = None;
                    }
                    self.emit(
                        e,
                        Event::Account(AccountState::Qr {
                            url: q.url,
                            status: poll.status,
                        }),
                    );
                }
            }
            Err(error) if self.current(e) => {
                self.login = None;
                self.emit(e, Event::Account(AccountState::Failed(error.to_string())));
            }
            _ => {}
        }
    }
    fn paused(&self) -> bool {
        self.queue.front().is_some_and(|p| p.delivery.blocked)
    }
    fn cancel_front(&mut self) {
        if let Some(mut p) = self.queue.pop_front() {
            p.delivery.state = DeliveryState::Cancelled;
            p.delivery.blocked = false;
            self.emit(p.epoch, Event::Delivery(p.delivery));
        }
    }
    fn cancel_stale(&mut self) {
        while self.queue.front().is_some_and(|p| {
            !self.current(p.epoch)
                || p.send_epoch != self.send_epoch.load(Ordering::Acquire)
                || !self.sending.load(Ordering::Acquire)
        }) {
            self.cancel_front();
        }
    }
    fn send_next(&mut self) {
        self.send_next_using(|client, room, text| client.send_danmaku(room, text));
    }
    fn send_next_using(
        &mut self,
        submit: impl FnOnce(&Client, u64, &str) -> Result<crate::SendReceipt, ApiError>,
    ) {
        if self.paused() || Instant::now() < self.next_send {
            return;
        }
        let Some(mut p) = self.queue.pop_front() else {
            return;
        };
        p.delivery.state = DeliveryState::Sending;
        self.emit(p.epoch, Event::Delivery(p.delivery.clone()));
        let result = submit(
            &self.client,
            p.delivery.room,
            &p.delivery.chunks[p.delivery.completed],
        );
        self.next_send = Instant::now() + Duration::from_secs(3);
        match result {
            Ok(receipt) => {
                p.delivery.completed += 1;
                if let Some(id) = receipt.id {
                    p.delivery.receipts.push(id);
                }
                if p.delivery.completed == p.delivery.chunks.len() {
                    p.delivery.state = DeliveryState::Submitted;
                } else {
                    p.delivery.state = DeliveryState::Queued;
                }
                self.emit(p.epoch, Event::Delivery(p.delivery.clone()));
                if p.delivery.state != DeliveryState::Submitted {
                    self.queue.push_front(p);
                }
            }
            Err(error) => {
                p.delivery.state = if matches!(
                    error,
                    ApiError::Business { .. } | ApiError::LoginRequired | ApiError::InvalidInput(_)
                ) {
                    DeliveryState::Failed
                } else {
                    DeliveryState::Uncertain
                };
                p.delivery.detail = error.to_string();
                p.delivery.blocked = true;
                self.emit(p.epoch, Event::Delivery(p.delivery.clone()));
                let request_epoch = p.epoch;
                self.queue.push_front(p);
                if is_auth_error(&error) && self.current(request_epoch) {
                    self.sending.store(false, Ordering::Release);
                    self.emit(request_epoch, Event::Account(AccountState::Expired));
                }
            }
        }
    }
}
fn is_auth_error(e: &ApiError) -> bool {
    matches!(
        e,
        ApiError::LoginRequired
            | ApiError::Business {
                code: -101 | -111,
                ..
            }
    )
}
fn receive(
    client: Client,
    room: Room,
    e: u64,
    epoch: Arc<AtomicU64>,
    alive: Arc<AtomicBool>,
    deliver: Deliver,
) {
    let current = || alive.load(Ordering::Acquire) && epoch.load(Ordering::Acquire) == e;
    let emit = |event| deliver(SessionEvent { epoch: e, event });
    while current() {
        emit(Event::Connection(ConnectionState::Connecting));
        let result = (|| {
            let info = client.danmu_info(room.id)?;
            if !current() {
                return Ok(());
            }
            let mut socket = LiveSocket::connect(&info)?;
            let mut heartbeat = Instant::now();
            let mut heard = Instant::now();
            let mut authenticated = false;
            while current() {
                if heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
                    socket.heartbeat()?;
                    heartbeat = Instant::now();
                }
                if heard.elapsed() > Duration::from_secs(60) {
                    return Err(ApiError::Network("chat heartbeat timed out".into()));
                }
                match socket.next_event(Duration::from_millis(200))? {
                    Some(LiveEvent::Authenticated) => {
                        authenticated = true;
                        heard = Instant::now();
                        emit(Event::Connection(ConnectionState::Connected(room.clone())));
                    }
                    Some(LiveEvent::Message(mut event)) => {
                        heard = Instant::now();
                        if authenticated {
                            match &mut event {
                                ChatEvent::Text { author, .. }
                                | ChatEvent::Gift { author, .. }
                                | ChatEvent::Member { author, .. }
                                | ChatEvent::SuperChat { author, .. } => {
                                    author.owner = author.uid == room.owner.to_string()
                                }
                                _ => {}
                            }
                            emit(Event::Chat {
                                room: room.id,
                                event,
                            });
                        }
                    }
                    Some(LiveEvent::Disconnected) => {
                        return Err(ApiError::Network("chat disconnected".into()));
                    }
                    Some(LiveEvent::Popularity { .. }) => heard = Instant::now(),
                    None => {}
                }
            }
            Ok::<_, ApiError>(())
        })();
        if !current() {
            return;
        }
        if let Err(error) = result {
            if is_auth_error(&error) {
                emit(Event::Account(AccountState::Expired));
                return;
            }
            emit(Event::Connection(ConnectionState::Error(error.to_string())));
        }
        for _ in 0..30 {
            if !current() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn worker() -> (Worker, Receiver<SessionEvent>) {
        let (_, rx) = unbounded();
        let (events, out) = unbounded();
        let client = Client::in_memory();
        client.import_cookies("SESSDATA=fixture; bili_jct=fixture; DedeUserID=7");
        (
            Worker {
                rx,
                epoch: Arc::new(AtomicU64::new(1)),
                send_epoch: Arc::new(AtomicU64::new(1)),
                sending: Arc::new(AtomicBool::new(true)),
                alive: Arc::new(AtomicBool::new(true)),
                deliver: Arc::new(move |e| {
                    let _ = events.send(e);
                }),
                store: Store::new(std::path::Path::new(".")),
                client,
                profile: Some(Profile {
                    uid: 7,
                    name: "test".into(),
                    avatar_url: String::new(),
                    room_id: Some(1),
                }),
                config: BilibiliConfig {
                    enabled: true,
                    room: Some(1),
                },
                room: Some(Room {
                    id: 1,
                    owner: 7,
                    title: String::new(),
                    max_chars: 3,
                }),
                queue: VecDeque::new(),
                seen: HashSet::new(),
                login: None,
                next_poll: Instant::now(),
                next_send: Instant::now(),
                loaded: true,
            },
            out,
        )
    }
    fn enqueue(w: &mut Worker, id: u64, text: &str) {
        w.command(Command::Send {
            permit: Permit::acquire(Arc::new(AtomicUsize::new(0))).unwrap(),
            epoch: 1,
            send_epoch: 1,
            line: LineId(id),
            text: text.into(),
        });
    }
    fn due(w: &mut Worker) {
        w.next_send = Instant::now();
    }
    #[test]
    fn capacity_counts_messages_waiting_in_the_mailbox() {
        let capacity = Arc::new(AtomicUsize::new(0));
        let mut permits: Vec<_> = (0..100)
            .map(|_| Permit::acquire(capacity.clone()).unwrap())
            .collect();
        assert!(Permit::acquire(capacity.clone()).is_none());
        permits.pop();
        assert!(Permit::acquire(capacity.clone()).is_some());
        drop(permits);
        assert_eq!(capacity.load(Ordering::Acquire), 0);
    }
    #[test]
    fn partial_failure_retries_only_the_unsent_chunk() {
        let (mut w, _) = worker();
        enqueue(&mut w, 1, "abcdef");
        enqueue(&mut w, 1, "abcdef");
        assert_eq!(w.queue.len(), 1);
        w.send_next_using(|_, room, text| {
            assert_eq!(room, 1);
            assert_eq!(text, "abc");
            Ok(crate::SendReceipt {
                id: Some("a".into()),
            })
        });
        due(&mut w);
        w.send_next_using(|_, _, text| {
            assert_eq!(text, "def");
            Err(ApiError::Business {
                code: 10031,
                message: "rate limited".into(),
            })
        });
        assert!(w.paused());
        assert_eq!(w.queue[0].delivery.completed, 1);
        w.send_next_using(|_, _, _| panic!("a failed write must not auto-retry"));
        w.command(Command::Retry(LineId(1)));
        due(&mut w);
        w.send_next_using(|_, _, text| {
            assert_eq!(text, "def");
            Ok(crate::SendReceipt {
                id: Some("b".into()),
            })
        });
        assert!(w.queue.is_empty());
    }
    #[test]
    fn uncertain_results_require_explicit_skip_and_never_retry() {
        let (mut w, out) = worker();
        enqueue(&mut w, 1, "first");
        enqueue(&mut w, 2, "next");
        w.send_next_using(|_, _, _| Err(ApiError::Network("lost response".into())));
        assert_eq!(w.queue[0].delivery.state, DeliveryState::Uncertain);
        w.command(Command::Retry(LineId(1)));
        assert!(w.paused());
        w.command(Command::SkipFailed(LineId(2)));
        assert_eq!(w.queue.len(), 2);
        w.command(Command::SkipFailed(LineId(1)));
        assert!(!w.paused());
        assert_eq!(w.queue[0].delivery.line, LineId(2));
        assert!(out.try_iter().any(|e| matches!(
            e.event,
            Event::Delivery(Delivery {
                state: DeliveryState::Uncertain,
                ..
            })
        )));
    }
    #[test]
    fn old_in_flight_rejection_cannot_expire_a_new_session() {
        let (mut w, out) = worker();
        enqueue(&mut w, 1, "old");
        let epoch = w.epoch.clone();
        w.send_next_using(|_, _, _| {
            epoch.store(2, Ordering::Release);
            Err(ApiError::LoginRequired)
        });
        assert!(w.sending.load(Ordering::Acquire));
        assert!(!out.try_iter().any(|e| matches!(e.event, Event::Account(_))));
        w.cancel_stale();
        assert!(w.queue.is_empty());
    }
    #[test]
    fn stale_qr_confirmation_cannot_replace_the_current_cookie_jar() {
        let (mut w, out) = worker();
        w.login = Some((
            1,
            crate::QrLogin {
                url: "fixture".into(),
                qrcode_key: "fixture".into(),
            },
            Instant::now(),
        ));
        let epoch = w.epoch.clone();
        w.poll_login_using(|client, _| {
            client.import_cookies("SESSDATA=new; bili_jct=new; DedeUserID=8");
            epoch.store(2, Ordering::Release);
            Ok(crate::QrPoll {
                status: QrStatus::Confirmed,
                message: String::new(),
            })
        });
        assert_eq!(w.client.mid(), Some(7));
        assert!(out.try_iter().next().is_none());
    }
    #[test]
    fn cancellation_and_queue_capacity_preserve_local_submissions() {
        let (mut w, out) = worker();
        for id in 0..101 {
            enqueue(&mut w, id, "hello");
        }
        assert_eq!(w.queue.len(), 100);
        assert!(out.try_iter().any(|e| matches!(
            e.event,
            Event::Delivery(Delivery {
                state: DeliveryState::Failed,
                blocked: false,
                ..
            })
        )));
        w.send_epoch.fetch_add(1, Ordering::AcqRel);
        w.cancel_stale();
        assert!(w.queue.is_empty());
        let (mut w, _) = worker();
        enqueue(&mut w, 1, "old");
        w.epoch.store(2, Ordering::Release);
        w.cancel_stale();
        assert!(w.queue.is_empty());
        enqueue(&mut w, 2, "stale");
        assert!(w.queue.is_empty());
    }
}
