//! File watching: a skin folder in hot-reload mode and sticker folders.
//! Bursts of events (an editor saving, a copy of fifty files) collapse into
//! one message after a short quiet period.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::msg::Msg;

const QUIET: Duration = Duration::from_millis(300);

pub struct Watch {
    /// Held only to keep watching: dropping it stops the watch.
    _watcher: Option<RecommendedWatcher>,
    paths: Vec<PathBuf>,
}

impl Watch {
    /// Watch `paths`; `msg` builds the message posted after changes settle.
    pub fn start(
        paths: Vec<PathBuf>,
        post: impl Fn(Msg) + Send + 'static,
        msg: fn() -> Msg,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<()>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|e| !e.kind.is_access()) {
                let _ = tx.send(());
            }
        });
        let mut watcher = match watcher {
            Ok(w) => w,
            Err(_) => {
                return Self {
                    _watcher: None,
                    paths,
                };
            }
        };
        for p in &paths {
            let _ = watcher.watch(p, RecursiveMode::Recursive);
        }
        thread::Builder::new()
            .name("nanatype-watch".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    // Wait for quiet.
                    while rx.recv_timeout(QUIET).is_ok() {}
                    post(msg());
                }
            })
            .expect("spawn watch thread");
        Self {
            _watcher: Some(watcher),
            paths,
        }
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}
