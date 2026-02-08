use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::model::{
  AppConfig,
  WatchConfig
};
use crate::ports::clock::Clock;
use crate::ports::http::Http;
use crate::ports::random::RandomSource;
use crate::ports::repo::Repo;

/// Bundles the runtime dependencies the
/// scheduler needs (configuration,
/// persistence, HTTP client, clock,
/// randomness source, and watch
/// metadata).
pub struct AppContext<R, H, C, G>
where
  R: Repo + ?Sized,
  H: Http,
  C: Clock,
  G: RandomSource
{
  pub cfg:           Arc<AppConfig>,
  pub repo:          Arc<R>,
  pub http:          Arc<H>,
  pub clock:         Arc<C>,
  pub rng:           Arc<G>,
  pub watches_by_id:
    Arc<HashMap<String, WatchConfig>>
}

impl<R, H, C, G> Clone
  for AppContext<R, H, C, G>
where
  R: Repo + ?Sized,
  H: Http,
  C: Clock,
  G: RandomSource
{
  fn clone(&self) -> Self {
    Self {
      cfg:           Arc::clone(
        &self.cfg
      ),
      repo:          Arc::clone(
        &self.repo
      ),
      http:          Arc::clone(
        &self.http
      ),
      clock:         Arc::clone(
        &self.clock
      ),
      rng:           Arc::clone(
        &self.rng
      ),
      watches_by_id: Arc::clone(
        &self.watches_by_id
      )
    }
  }
}
