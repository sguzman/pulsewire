use std::path::{Path, PathBuf};

use rssify::app::{context::AppContext, scheduler::Scheduler};
use rssify::infra::{
  config::ConfigLoader,
  logging::init_logging,
  random::MutexRng,
  reqwest_http::ReqwestHttp,
  sqlite_repo::SqliteRepo,
  system_clock::SystemClock,
};
use rssify::domain::model::AppMode;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), rssify::infra::logging::BootError> {
  init_logging();

  let cfg_path = pick_config_path(std::env::args().skip(1).next());
  let cfg = ConfigLoader::load(&cfg_path).await?;

  info!(timezone = %cfg.timezone, "Using timezone");
  info!(
    feeds = cfg.feeds.len(),
    db_path = %cfg.db_path.display(),
    mode = ?cfg.mode,
    "Loaded config"
  );

  if matches!(cfg.mode, AppMode::Dev) {
    warn!(db_path = %cfg.db_path.display(), "Dev mode enabled, deleting database");
    let _ = tokio::fs::remove_file(&cfg.db_path).await;
  }

  let repo = SqliteRepo::new(&cfg.db_path).await?;
  repo.migrate(&cfg.timezone).await?;
  repo.upsert_feeds(&cfg.feeds, &cfg.timezone).await?;

  let http = ReqwestHttp::new(cfg.user_agent.clone())?;
  let clock = SystemClock::default();
  let rng = MutexRng::new();

  let ctx = AppContext {
    cfg,
    repo,
    http,
    clock,
    rng,
  };

  if let Err(e) = Scheduler::run_forever(ctx).await {
    error!(error = %e, "Fatal error");
    return Err(rssify::infra::logging::BootError::Fatal(e.to_string()));
  }

  Ok(())
}

fn pick_config_path(arg1: Option<String>) -> PathBuf {
  // Scala default: os.pwd / src/main/resources/config/config.toml :contentReference[oaicite:1]{index=1}
  let default = PathBuf::from("src/main/resources/config/config.toml");
  match arg1 {
    Some(p) => PathBuf::from(p),
    None => default,
  }
}
