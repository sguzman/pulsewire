//! `Clock` implementation backed by `SystemTime`.
use crate::ports::clock::Clock;

#[derive(Default)]
pub struct SystemClock;

#[async_trait::async_trait]
impl Clock for SystemClock {
    async fn now_epoch_ms(&self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        now.as_millis() as i64
    }
}
