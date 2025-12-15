//! Clock abstraction (epoch milliseconds).
#[async_trait::async_trait]
pub trait Clock: Send + Sync {
    async fn now_epoch_ms(&self) -> i64;
}
