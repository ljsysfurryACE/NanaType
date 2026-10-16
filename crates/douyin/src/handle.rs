use crate::{
    AccountState, Config, ConnectionState, Event, SessionEvent, credentials::Store, native,
};
use crossbeam_channel::{Sender, unbounded};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type Deliver = Arc<dyn Fn(SessionEvent) + Send + Sync>;
#[derive(Clone)]
pub(crate) struct Session {
    pub room: String,
    epoch: u64,
    revision: Arc<AtomicU64>,
    alive: Arc<AtomicBool>,
    deliver: Deliver,
}
impl Session {
    #[cfg(test)]
    pub fn fixture(deliver: impl Fn(SessionEvent) + Send + Sync + 'static) -> Self {
        Self {
            room: "123".into(),
            epoch: 1,
            revision: Arc::new(AtomicU64::new(1)),
            alive: Arc::new(AtomicBool::new(true)),
            deliver: Arc::new(deliver),
        }
    }
    #[cfg(test)]
    pub fn stop(&self) {
        self.alive.store(false, Ordering::Release);
    }
    pub fn current(&self) -> bool {
        self.alive.load(Ordering::Acquire) && self.revision.load(Ordering::Acquire) == self.epoch
    }
    pub fn emit(&self, event: Event) {
        if self.current() {
            (self.deliver)(SessionEvent {
                epoch: self.epoch,
                event,
            });
        }
    }
    fn error(&self, code: impl Into<String>) {
        self.emit(Event::Connection(ConnectionState::Error(code.into())));
    }
    pub fn pause(&self, duration: Duration) -> bool {
        let end = Instant::now() + duration;
        while self.current() && Instant::now() < end {
            thread::sleep(
                Duration::from_millis(25).min(end.saturating_duration_since(Instant::now())),
            );
        }
        self.current()
    }
}
enum Command {
    Run(Config),
    Import(String),
    Logout,
    Stop,
}
pub struct DouyinHandle {
    tx: Sender<(u64, Command)>,
    revision: Arc<AtomicU64>,
    alive: Arc<AtomicBool>,
    config: Config,
    controller: Option<JoinHandle<()>>,
}
impl DouyinHandle {
    pub fn new(
        config_dir: PathBuf,
        deliver: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = unbounded();
        let revision = Arc::new(AtomicU64::new(0));
        let alive = Arc::new(AtomicBool::new(true));
        let callback: Deliver = Arc::new(deliver);
        let controller = thread::Builder::new()
            .name("nanatype-douyin".into())
            .spawn({
                let revision = revision.clone();
                let alive = alive.clone();
                move || {
                    let store = Store::new(&config_dir);
                    let mut worker: Option<JoinHandle<()>> = None;
                    for (epoch, command) in rx {
                        if let Some(worker) = worker.take() {
                            let _ = worker.join();
                        }
                        let mut session = Session {
                            room: String::new(),
                            epoch,
                            revision: revision.clone(),
                            alive: alive.clone(),
                            deliver: callback.clone(),
                        };
                        match command {
                            Command::Run(config) => {
                                if !session.current() {
                                    continue;
                                }
                                if !config.enabled {
                                    session.emit(Event::Connection(ConnectionState::Off));
                                    continue;
                                }
                                session.room = match crate::room_id(&config.room) {
                                    Ok(room) => room,
                                    Err(error) => {
                                        session.error(error);
                                        continue;
                                    }
                                };
                                let cookie = match store.load() {
                                    Ok(bytes) => bytes
                                        .and_then(|b| String::from_utf8(b).ok())
                                        .unwrap_or_default(),
                                    Err(_) => {
                                        session
                                            .emit(Event::Warning("protected-storage-unavailable"));
                                        String::new()
                                    }
                                };
                                let active = session.clone();
                                worker = thread::Builder::new()
                                    .name("nanatype-douyin-session".into())
                                    .spawn(move || {
                                        if let Err(error) = native::run(&active, &cookie) {
                                            active.error(error);
                                        }
                                    })
                                    .ok();
                                if worker.is_none() {
                                    session.error("worker-start-failed");
                                }
                            }
                            Command::Import(cookie) => {
                                if !session.current() {
                                    continue;
                                }
                                session.emit(Event::Connection(ConnectionState::Off));
                                let validation = native::Client::new(&cookie)
                                    .and_then(|mut client| client.validate());
                                // A room/account change while verification was in flight must not save its credentials.
                                if !session.current() {
                                    continue;
                                }
                                match validation {
                                    Ok(account @ AccountState::Ready { .. }) => match store
                                        .save(cookie.as_bytes())
                                    {
                                        Ok(()) => session.emit(Event::Account(account)),
                                        Err(_) => session.error("protected-storage-unavailable"),
                                    },
                                    Ok(account) => {
                                        session.emit(Event::Account(account));
                                        session.error("account-unverified");
                                    }
                                    Err(error) => {
                                        session.emit(Event::Account(AccountState::Unverified));
                                        session.error(error);
                                    }
                                }
                            }
                            Command::Logout => {
                                let result = store.clear();
                                session.emit(Event::Account(AccountState::SignedOut));
                                match result {
                                    Ok(()) => session.emit(Event::Connection(ConnectionState::Off)),
                                    Err(_) => session.error("credentials-clear-failed"),
                                }
                            }
                            Command::Stop => break,
                        }
                    }
                }
            })
            .expect("start Douyin service");
        Self {
            tx,
            revision,
            alive,
            config: Config::default(),
            controller: Some(controller),
        }
    }
    pub fn epoch(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }
    fn advance(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::AcqRel) + 1
    }
    pub fn set(&mut self, config: Config) {
        if self.config == config {
            return;
        }
        self.config = config;
        self.refresh();
    }
    pub fn refresh(&self) {
        let _ = self
            .tx
            .send((self.advance(), Command::Run(self.config.clone())));
    }
    pub fn import_cookie(&self, cookie: String) {
        let _ = self.tx.send((self.advance(), Command::Import(cookie)));
    }
    pub fn logout(&self) {
        let _ = self.tx.send((self.advance(), Command::Logout));
    }
}
impl Drop for DouyinHandle {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        let _ = self.tx.send((self.advance(), Command::Stop));
        if let Some(controller) = self.controller.take() {
            let _ = controller.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importing_an_anonymous_cookie_does_not_store_or_confirm_an_account() {
        let directory = tempfile::tempdir().unwrap();
        let (tx, rx) = unbounded();
        let handle = DouyinHandle::new(directory.path().into(), move |event| {
            tx.send(event).unwrap();
        });
        handle.import_cookie("ttwid=anonymous".into());
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut expired = false;
        loop {
            let event = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            assert_eq!(event.epoch, handle.epoch());
            match event.event {
                Event::Account(AccountState::Ready { .. }) => {
                    panic!("anonymous cookie verified as an account")
                }
                Event::Account(AccountState::Expired) => expired = true,
                Event::Connection(ConnectionState::Error(code)) => {
                    assert_eq!(code, "account-unverified");
                    break;
                }
                _ => {}
            }
        }
        assert!(expired);
        assert!(Store::new(directory.path()).load().unwrap().is_none());
    }
}
