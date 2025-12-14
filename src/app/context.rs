use crate::domain::model::AppConfig;
use crate::ports::{clock::Clock, http::Http, random::RandomSource, repo::Repo};

pub struct AppContext<R, H, C, G>
where
    R: Repo,
    H: Http,
    C: Clock,
    G: RandomSource,
{
    pub cfg: AppConfig,
    pub repo: R,
    pub http: H,
    pub clock: C,
    pub rng: G,
}
