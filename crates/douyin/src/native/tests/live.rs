//! Real TLS data passes through a local byte relay; no TLS termination or browser.
use super::super::*;
use std::{
    collections::HashSet,
    io::{Read, Write},
    net::{Shutdown, TcpListener},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
};

#[derive(Default)]
struct Gate {
    offline: AtomicBool,
    blocked: AtomicUsize,
    tunnels: AtomicUsize,
    bytes: AtomicU64,
    sockets: Mutex<Vec<TcpStream>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}
impl Gate {
    fn open(self: &Arc<Self>, host: &str, port: u16) -> Result<TcpStream, String> {
        if self.offline.load(Ordering::Acquire) {
            self.blocked.fetch_add(1, Ordering::Relaxed);
            return Err("test-network-offline".into());
        }
        let remote = dial(host, port)?;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (local, _) = listener.accept().unwrap();
        local.set_nodelay(true).unwrap();
        remote.set_nodelay(true).unwrap();
        self.sockets
            .lock()
            .unwrap()
            .extend([local.try_clone().unwrap(), remote.try_clone().unwrap()]);
        self.tunnels.fetch_add(1, Ordering::Relaxed);
        for (mut reader, mut writer, count) in [
            (
                local.try_clone().unwrap(),
                remote.try_clone().unwrap(),
                false,
            ),
            (remote, local, true),
        ] {
            let gate = self.clone();
            self.workers.lock().unwrap().push(thread::spawn(move || {
                let mut buffer = [0u8; 16384];
                while let Ok(length) = reader.read(&mut buffer) {
                    if length == 0 || writer.write_all(&buffer[..length]).is_err() {
                        break;
                    }
                    if count {
                        gate.bytes.fetch_add(length as u64, Ordering::Relaxed);
                    }
                }
                let _ = reader.shutdown(Shutdown::Both);
                let _ = writer.shutdown(Shutdown::Both);
            }));
        }
        Ok(client)
    }
    fn set_offline(&self, offline: bool) {
        self.offline.store(offline, Ordering::Release);
        if offline {
            for socket in self.sockets.lock().unwrap().drain(..) {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
    }
    fn finish(&self) {
        self.set_offline(true);
        for worker in self.workers.lock().unwrap().drain(..) {
            worker.join().unwrap();
        }
    }
}
struct Running {
    session: Session,
    worker: Option<JoinHandle<Result<(), String>>>,
    gate: Arc<Gate>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.session.stop();
        self.gate.set_offline(true);
        if let Some(worker) = self.worker.take() {
            assert!(worker.join().unwrap().is_ok());
        }
        self.gate.finish();
    }
}

#[test]
#[ignore = "real native live room, TLS byte-stream outage and 2-minute recovery acceptance"]
fn native_live_network_acceptance() {
    let room = std::env::var("NANATYPE_DOUYIN_ROOM")
        .expect("set NANATYPE_DOUYIN_ROOM to a live public room");
    let seconds = std::env::var("NANATYPE_DOUYIN_NATIVE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(120)
        .max(120);
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut session = Session::fixture(move |event| {
        let _ = tx.send(event);
    });
    session.room = crate::room_id(&room).unwrap();
    let gate = Arc::new(Gate::default());
    let mut client = Client::new("").unwrap();
    let dial_gate = gate.clone();
    client.dialer = Some(Box::new(move |host, port| dial_gate.open(host, port)));
    let worker_session = session.clone();
    let running = Running {
        session,
        worker: Some(thread::spawn(move || receive(&worker_session, client))),
        gate: gate.clone(),
    };
    let started = Instant::now();
    let mut first = None;
    let mut restored_at = None;
    let mut recovered_at = None;
    let mut interrupted = false;
    let mut messages = 0;
    let mut after_recovery = 0;
    let mut ids = HashSet::new();
    let mut duplicates = 0;
    let mut minute_counts = vec![0usize; seconds.div_ceil(60) as usize];
    let mut last_report = 0;
    let mut connected = 0;
    eprintln!("native live room={room}: no browser, anonymous HTTP + signed TLS WebSocket");
    loop {
        let elapsed = started.elapsed().as_secs();
        assert!(elapsed < seconds + 240, "native acceptance timeout");
        if first.is_some_and(|at| elapsed >= at + 30) && !interrupted {
            gate.set_offline(true);
            interrupted = true;
            eprintln!(
                "t={elapsed}s native data network OFFLINE: sockets shut down, new dials blocked"
            );
        }
        if interrupted && restored_at.is_none() && first.is_some_and(|at| elapsed >= at + 50) {
            gate.set_offline(false);
            restored_at = Some(elapsed);
            eprintln!("t={elapsed}s native data network ONLINE; no refresh/restart");
        }
        if let Ok(event) = rx.recv_timeout(Duration::from_millis(200)) {
            match event.event {
                Event::Chat { event, .. } => {
                    first.get_or_insert(elapsed);
                    messages += 1;
                    if !matches!(event.content, crate::Content::Gift { .. })
                        && !ids.insert(event.id)
                    {
                        duplicates += 1;
                    }
                    if restored_at.is_some() {
                        let at = *recovered_at.get_or_insert(elapsed);
                        after_recovery += 1;
                        if let Some(count) = minute_counts.get_mut(((elapsed - at) / 60) as usize) {
                            *count += 1;
                        }
                    }
                }
                Event::Connection(state) => {
                    eprintln!("t={elapsed}s connection={state:?}");
                    assert!(!matches!(
                        state,
                        ConnectionState::Error(_)
                            | ConnectionState::Disconnected
                            | ConnectionState::Off
                    ));
                    if let ConnectionState::Connected { .. } = state {
                        connected += 1;
                    }
                }
                Event::Warning(code) => panic!("unexpected native warning: {code}"),
                _ => {}
            }
        }
        assert!(
            first.is_some() || elapsed < 90,
            "no initial native messages within 90s"
        );
        if let Some(at) = restored_at {
            assert!(
                recovered_at.is_some() || elapsed < at + 90,
                "no native automatic network recovery within 90s"
            );
        }
        if elapsed / 15 > last_report {
            last_report = elapsed / 15;
            eprintln!(
                "t={elapsed}s messages={messages} after_recovery={after_recovery} blocked_dials={} tunnels={} tls_bytes={}",
                gate.blocked.load(Ordering::Relaxed),
                gate.tunnels.load(Ordering::Relaxed),
                gate.bytes.load(Ordering::Relaxed)
            );
        }
        if recovered_at.is_some_and(|at| elapsed >= at + seconds) {
            break;
        }
    }
    assert!(
        gate.blocked.load(Ordering::Relaxed) > 0,
        "no blocked reconnect attempts during outage"
    );
    assert!(
        gate.tunnels.load(Ordering::Relaxed) >= 2 && connected >= 2,
        "no fresh verified native connection"
    );
    assert_eq!(duplicates, 0);
    assert!(minute_counts.iter().all(|n| *n > 0));
    eprintln!(
        "ACCEPTED_NATIVE room={room} duration={}s stable_after_recovery={seconds}s messages={messages} after_recovery={after_recovery} recovery_delay={}s blocked_dials={} verified_connections={connected} duplicate_non_gift={duplicates} recovery_minute_counts={minute_counts:?}",
        started.elapsed().as_secs(),
        recovered_at.unwrap() - restored_at.unwrap(),
        gate.blocked.load(Ordering::Relaxed)
    );
    drop(running);
}
