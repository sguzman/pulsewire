//! Random source abstraction (0-1 floats).
#[async_trait::async_trait]
pub trait RandomSource: Send + Sync {
    async fn next_f64(&self) -> f64; // expected in [0,1)
}
