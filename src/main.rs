use std::path::PathBuf;

use feedrv3::app::{context::AppContext, scheduler::Scheduler};
use feedrv3::infra::{
  config::ConfigLoader,
  logging::{init_logging, BootError},
  random::MutexRng,
  reqwest_http::ReqwestHttp,
  sqlite_repo::SqliteRepo,
  system_clock::SystemClock,
};
use feedrv3::domain::model::AppMode;
use feedrv3::ports::repo::Repo;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), BootError> {
  let cfg_path = pick_config_path(std::env::args().skip(1).next());
  let cfg = ConfigLoader::load(&cfg_path).await.map_err(|e| BootError::Fatal(e.to_string()))?;
  init_logging(&cfg.log_level);

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

  let repo = SqliteRepo::new(&cfg.db_path).await.map_err(BootError::Fatal)?;
  repo.migrate(&cfg.timezone).await.map_err(BootError::Fatal)?;
  repo.upsert_feeds(&cfg.feeds, &cfg.timezone).await.map_err(BootError::Fatal)?;

  let http = ReqwestHttp::new(cfg.user_agent.clone()).map_err(|e| BootError::Fatal(e.to_string()))?;
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
    return Err(BootError::Fatal(e.to_string()));
  }

  Ok(())
}

fn pick_config_path(arg1: Option<String>) -> PathBuf {
  if let Some(p) = arg1 {
    return PathBuf::from(p);
  }

  // Prefer repo-local res/ config; fall back to old resources path for compatibility.
  let candidates = [
    PathBuf::from("res/config.toml"),
    PathBuf::from("src/main/resources/config/config.toml"),
  ];
  for p in &candidates {
    if p.exists() {
      return p.clone();
    }
  }
  candidates[0].clone()
}
