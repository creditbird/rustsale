//! Fixed one-minute window per client. Counts every authenticated request.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const REQUESTS_PER_MINUTE: u32 = 60;
pub const SENDS_PER_MINUTE: u32 = 10;
const WINDOW: Duration = Duration::from_secs(60);

struct Window {
    start: Instant,
    requests: u32,
    sends: u32,
}

#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, Window>>,
}

impl RateLimiter {
    /// `Err(retry_after_secs)` when the client is over its budget.
    pub fn check(&self, client: &str, is_send: bool) -> Result<(), u64> {
        let mut map = match self.windows.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        let window = map.entry(client.to_string()).or_insert(Window {
            start: now,
            requests: 0,
            sends: 0,
        });
        if now.duration_since(window.start) >= WINDOW {
            *window = Window {
                start: now,
                requests: 0,
                sends: 0,
            };
        }
        let retry = WINDOW
            .saturating_sub(now.duration_since(window.start))
            .as_secs()
            .max(1);
        if window.requests >= REQUESTS_PER_MINUTE {
            return Err(retry);
        }
        window.requests += 1;
        if is_send {
            if window.sends >= SENDS_PER_MINUTE {
                return Err(retry);
            }
            window.sends += 1;
        }
        Ok(())
    }
}
