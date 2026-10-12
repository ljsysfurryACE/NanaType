//! The connection thread: join, keep alive, reconnect.

use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use tungstenite::{Message, WebSocket};

use crate::protocol::{self, ChatEvent, FatalKind, Incoming, RoomKey};

/// blivechat sends a heartbeat every 10 s and drops clients silent for 15 s.
const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);
/// No frame at all for this long means the connection is dead.
const SILENCE_LIMIT: Duration = Duration::from_secs(30);
const RETRY_AFTER: Duration = Duration::from_secs(3);
const RETRY_AFTER_FATAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BliveConfig {
    pub host: String,
    pub port: u16,
    pub room: RoomKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BliveStatus {
    Off,
    Connecting,
    /// Joined the room.
    Connected,
    /// The connection failed or dropped; it is retried.
    Error(String),
    /// blivechat gave up on the room. An auth code error is not retried.
    Fatal(FatalKind, String),
}

type StatusFn = Arc<dyn Fn(BliveStatus) + Send + Sync>;
type EventFn = Arc<dyn Fn(ChatEvent) + Send + Sync>;

struct Running {
    config: BliveConfig,
    _stop: Sender<()>,
}

/// Owns the connection thread. Dropping it (or `set(None)`) stops the thread.
pub struct BliveHandle {
    status: StatusFn,
    events: EventFn,
    running: Option<Running>,
}

impl BliveHandle {
    pub fn new(
        status: impl Fn(BliveStatus) + Send + Sync + 'static,
        events: impl Fn(ChatEvent) + Send + Sync + 'static,
    ) -> Self {
        Self {
            status: Arc::new(status),
            events: Arc::new(events),
            running: None,
        }
    }

    /// Connect with `config`, reconnect after a change, or stop on `None`.
    pub fn set(&mut self, config: Option<BliveConfig>) {
        if self.running.as_ref().map(|r| &r.config) == config.as_ref() {
            return;
        }
        self.running = None;
        let Some(config) = config else {
            (self.status)(BliveStatus::Off);
            return;
        };
        let (stop_tx, stop_rx) = unbounded::<()>();
        let cfg = config.clone();
        let status = Arc::clone(&self.status);
        let events = Arc::clone(&self.events);
        thread::Builder::new()
            .name("nanatype-blive".into())
            .spawn(move || run(cfg, stop_rx, status, events))
            .expect("spawn blivechat thread");
        self.running = Some(Running {
            config,
            _stop: stop_tx,
        });
    }

    pub fn config(&self) -> Option<&BliveConfig> {
        self.running.as_ref().map(|r| &r.config)
    }
}

fn connect(config: &BliveConfig) -> Result<WebSocket<TcpStream>, String> {
    let addr = (config.host.as_str(), config.port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| format!("{} has no address", config.host))?;
    let stream =
        TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    let url = format!("ws://{}:{}/api/chat", config.host, config.port);
    let (socket, _) = tungstenite::client(url, stream).map_err(|e| e.to_string())?;
    socket
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;
    Ok(socket)
}

fn stopped(stop: &Receiver<()>) -> bool {
    stop.try_recv() != Err(TryRecvError::Empty)
}

/// Sleep until `until`, waking early when stopped.
fn wait(stop: &Receiver<()>, until: Instant) -> bool {
    let left = until.saturating_duration_since(Instant::now());
    !matches!(
        stop.recv_timeout(left),
        Err(crossbeam_channel::RecvTimeoutError::Timeout)
    )
}

fn run(config: BliveConfig, stop: Receiver<()>, status: StatusFn, events: EventFn) {
    loop {
        if stopped(&stop) {
            return;
        }
        status(BliveStatus::Connecting);
        let retry = match session(&config, &stop, &status, &events) {
            Ended::Stopped => return,
            Ended::Dropped(reason) => {
                status(BliveStatus::Error(reason));
                RETRY_AFTER
            }
            Ended::Fatal(FatalKind::AuthCode, message) => {
                status(BliveStatus::Fatal(FatalKind::AuthCode, message));
                // Wait for a new configuration.
                let _ = stop.recv();
                return;
            }
            Ended::Fatal(kind, message) => {
                status(BliveStatus::Fatal(kind, message));
                RETRY_AFTER_FATAL
            }
        };
        if wait(&stop, Instant::now() + retry) {
            return;
        }
    }
}

enum Ended {
    Stopped,
    Dropped(String),
    Fatal(FatalKind, String),
}

fn session(
    config: &BliveConfig,
    stop: &Receiver<()>,
    status: &StatusFn,
    events: &EventFn,
) -> Ended {
    let mut socket = match connect(config) {
        Ok(socket) => socket,
        Err(e) => return Ended::Dropped(e),
    };
    if let Err(e) = socket.send(Message::text(protocol::join_message(&config.room))) {
        return Ended::Dropped(e.to_string());
    }
    status(BliveStatus::Connected);
    let mut last_sent = Instant::now();
    let mut last_heard = Instant::now();
    loop {
        if stopped(stop) {
            let _ = socket.close(None);
            let _ = socket.flush();
            return Ended::Stopped;
        }
        if last_sent.elapsed() >= HEARTBEAT_EVERY {
            if let Err(e) = socket.send(Message::text(protocol::heartbeat_message())) {
                return Ended::Dropped(e.to_string());
            }
            last_sent = Instant::now();
        }
        if last_heard.elapsed() >= SILENCE_LIMIT {
            return Ended::Dropped("no data from blivechat".into());
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                last_heard = Instant::now();
                match protocol::parse(text.as_str()) {
                    Some(Incoming::Heartbeat) => {
                        if let Err(e) = socket.send(Message::text(protocol::heartbeat_message())) {
                            return Ended::Dropped(e.to_string());
                        }
                        last_sent = Instant::now();
                    }
                    Some(Incoming::Event(ChatEvent::Fatal { kind, message })) => {
                        let _ = socket.close(None);
                        return Ended::Fatal(kind, message);
                    }
                    Some(Incoming::Event(event)) => events(event),
                    Some(Incoming::Other) | None => {}
                }
            }
            Ok(Message::Close(_)) => {
                return Ended::Dropped("blivechat closed the connection".into());
            }
            Ok(_) => last_heard = Instant::now(),
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Ended::Dropped(e.to_string()),
        }
    }
}
