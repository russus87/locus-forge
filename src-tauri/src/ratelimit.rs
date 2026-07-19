use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::sleep;

/// Token-bucket per singola sorgente.
struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_per_sec: f64,
    last: Instant,
}

impl TokenBucket {
    fn new(rate_per_sec: f64) -> Self {
        let cap = rate_per_sec.max(1.0);
        Self {
            tokens: cap,
            capacity: cap,
            refill_per_sec: rate_per_sec,
            last: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
    }

    fn try_acquire(&mut self) -> Option<Duration> {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            None
        } else {
            let deficit = 1.0 - self.tokens;
            Some(Duration::from_secs_f64((deficit / self.refill_per_sec).max(0.005)))
        }
    }
}

/// Rate limiter con un bucket per sorgente + backoff esponenziale.
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, TokenBucket>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Attende finché un token è disponibile per la sorgente indicata.
    pub async fn acquire(&self, source: &str, rate_per_sec: f64) {
        loop {
            let wait = {
                let mut map = self.buckets.lock().await;
                let bucket = map
                    .entry(source.to_string())
                    .or_insert_with(|| TokenBucket::new(rate_per_sec));
                bucket.try_acquire()
            };
            match wait {
                None => return,
                Some(d) => sleep(d).await,
            }
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
