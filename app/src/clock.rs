//! One monotonic clock for the session, in milliseconds since launch.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

static START: OnceLock<(Instant, u64)> = OnceLock::new();

fn origin() -> (Instant, u64) {
    *START.get_or_init(|| (Instant::now(), nanatype_model::unix_millis()))
}

fn start() -> Instant {
    origin().0
}

/// Unix milliseconds of the monotonic time `ms` (for history timestamps).
pub fn unix_of(ms: u64) -> u64 {
    origin().1 + ms
}

pub fn now_ms() -> u64 {
    start().elapsed().as_millis() as u64
}

pub fn instant(ms: u64) -> Instant {
    start() + Duration::from_millis(ms)
}
