use std::sync::Arc;

use crate::domain::model::AppConfig;
use crate::ports::{clock::Clock, http::Http, random::RandomSource, repo::Repo};

/// Bundles the runtime dependencies the scheduler needs (configuration,
/// persistence, HTTP client, clock, and randomness source).
pub struct AppContext<R, H, C, G>
where
    R: Repo + ?Sized,
    H: Http,
    C: Clock,
    G: RandomSource,
{
    pub cfg: Arc<AppConfig>,
    pub repo: Arc<R>,
    pub http: Arc<H>,
    pub clock: Arc<C>,
    pub rng: Arc<G>,
}

impl<R, H, C, G> Clone for AppContext<R, H, C, G>
where
    R: Repo + ?Sized,
    H: Http,
    C: Clock,
    G: RandomSource,
{
    fn clone(&self) -> Self {
        Self {
            cfg: Arc::clone(&self.cfg),
            repo: Arc::clone(&self.repo),
            http: Arc::clone(&self.http),
            clock: Arc::clone(&self.clock),
            rng: Arc::clone(&self.rng),
        }
    }
}
