//! Mutex-protected RNG implementing the `RandomSource` port.
use crate::ports::random::RandomSource;
use rand::Rng;
use tokio::sync::Mutex;

pub struct MutexRng {
    inner: Mutex<rand::rngs::StdRng>,
}

impl MutexRng {
    pub fn new() -> Self {
        let seed = rand::rng().random::<[u8; 32]>();
        Self {
            inner: Mutex::new(rand::SeedableRng::from_seed(seed)),
        }
    }
}

#[async_trait::async_trait]
impl RandomSource for MutexRng {
    async fn next_f64(&self) -> f64 {
        let mut g = self.inner.lock().await;
        g.random::<f64>()
    }
}
