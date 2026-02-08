use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pulsewire_core::app::context::AppContext;
use pulsewire_core::app::scheduler::Scheduler;
use pulsewire_core::domain::model::{
  AppConfig,
  AppMode,
  FeedConfig,
  SqlDialect,
  WatchConfig,
};
use pulsewire_core::infra::config::{
  ConfigLoader,
  LoadedConfig,
};
use pulsewire_core::infra::logging::{
  init_logging,
  BootError,
};
use pulsewire_core::infra::random::MutexRng;
use pulsewire_core::infra::reqwest_http::ReqwestHttp;
use pulsewire_core::infra::system_clock::SystemClock;
use pulsewire_core::infra::{
  database,
  metrics,
};
use pulsewire_core::ports::repo::Repo;
use tracing::{
  error,
  info,
  warn,
};

/// Binary entrypoint:
/// - parses CLI args (`CONFIG_PATH` or
///   `--ingest-benchmark N`)
/// - loads TOML config bundle
///   (app/domains/feeds), initializes
///   logging
/// - optionally wipes the DB in dev,
///   opens SQLite + runs migrations
/// - bulk upserts feeds, then either
///   runs the ingest benchmark
///   (HEAD/GET skipped) or starts the
///   scheduler loop with
///   HTTP/clock/rng/repo adapters
/// - exits with `BootError` on fatal
///   startup/ingest errors
#[tokio::main]
async fn main() -> Result<(), BootError>
{
  let args = parse_args();

  let cfg_path =
    pick_config_path(args.config_path);

  let LoadedConfig {
    app: app_cfg,
    feeds,
    watches,
    categories
  } = ConfigLoader::load(&cfg_path)
    .await
    .map_err(|e| {
      BootError::Fatal(e.to_string())
    })?;

  let watch_feed_configs =
    watches_to_feeds(&watches);

  let mut all_sources = feeds;
  all_sources
    .extend(watch_feed_configs.clone());

  let watches_by_id = Arc::new(
    watches
      .into_iter()
      .map(|w| (w.id.clone(), w))
      .collect::<HashMap<_, _>>()
  );

  init_logging(&app_cfg);

  metrics::init(
    &app_cfg.metrics,
    &categories
  )
  .await
  .map_err(BootError::Fatal)?;

  info!(timezone = %app_cfg.timezone, "Using timezone");

  let db_desc = match app_cfg.db_dialect
  {
    | SqlDialect::Sqlite => {
      format!(
        "sqlite:{}",
        app_cfg.sqlite_path.display()
      )
    }
    | SqlDialect::Postgres => {
      format!(
        "postgres://{}@{}:{}/{}",
        app_cfg.postgres.user,
        app_cfg.postgres.host,
        app_cfg.postgres.port,
        app_cfg.postgres.database
      )
    }
  };

  info!(feeds = all_sources.len(), watches = watches_by_id.len(), db = %db_desc, dialect = ?app_cfg.db_dialect, mode = ?app_cfg.mode, "Loaded config");

  if !watches_by_id.is_empty() {
    info!(
      watches = watches_by_id.len(),
      "Ad-hoc watches enabled"
    );
  }

  if matches!(
    app_cfg.mode,
    AppMode::Dev
  ) {
    match app_cfg.db_dialect {
      | SqlDialect::Sqlite => {
        warn!(
            db_path = %app_cfg.sqlite_path.display(),
            "Dev mode enabled, deleting database"
        );

        let _ = tokio::fs::remove_file(
          &app_cfg.sqlite_path
        )
        .await;
      }
      | SqlDialect::Postgres => {
        warn!(
            db = %app_cfg.postgres.database,
            host = %app_cfg.postgres.host,
            port = app_cfg.postgres.port,
            "Dev mode enabled, wiping database"
        );

        pulsewire_core::infra::postgres_repo::wipe_database(
                &app_cfg.postgres,
                &app_cfg.timezone,
            )
            .await
            .map_err(BootError::Fatal)?;
      }
    }
  }

  let repo = database::create_repo(
    app_cfg.db_dialect,
    &app_cfg
  )
  .await
  .map_err(BootError::Fatal)?;

  repo
    .migrate(
      &app_cfg.timezone,
      app_cfg.default_poll_seconds
    )
    .await
    .map_err(BootError::Fatal)?;

  let cfg = Arc::new(app_cfg);

  let category_names: Vec<String> =
    categories
      .iter()
      .map(|c| c.name.clone())
      .collect();

  match args.mode {
    | RunMode::IngestBenchmark {
      feeds_to_insert
    } => {
      if feeds_to_insert == 0 {
        return Err(BootError::Fatal(
          "ingest benchmark requires \
           a feed count > 0"
            .into()
        ));
      }

      info!(
        feeds = feeds_to_insert,
        "Starting ingest benchmark \
         only"
      );

      repo
        .upsert_categories(
          vec!["benchmark".to_string()],
          &cfg.timezone
        )
        .await
        .map_err(BootError::Fatal)?;

      ingest_feeds(
        repo.clone(),
        cfg.clone(),
        benchmark_feed_stream(
          feeds_to_insert,
          cfg.default_poll_seconds,
          "benchmark".to_string()
        )
      )
      .await?;

      info!(
        feeds = feeds_to_insert,
        "Ingest benchmark finished"
      );

      return Ok(());
    }
    | RunMode::Scheduler => {}
  }

  repo
    .upsert_categories(
      category_names.clone(),
      &cfg.timezone
    )
    .await
    .map_err(BootError::Fatal)?;

  ingest_feeds(
    repo.clone(),
    cfg.clone(),
    all_sources
  )
  .await?;

  let http = Arc::new(
    ReqwestHttp::new(
      cfg.user_agent.clone()
    )
    .map_err(|e| {
      BootError::Fatal(e.to_string())
    })?
  );

  let clock = Arc::new(SystemClock);

  let rng = Arc::new(MutexRng::new());

  let ctx = AppContext {
    cfg: cfg.clone(),
    repo: repo.clone(),
    http: http.clone(),
    clock: clock.clone(),
    rng: rng.clone(),
    watches_by_id
  };

  if let Err(e) =
    Scheduler::run_forever_by_category(
      ctx,
      category_names
    )
    .await
  {
    error!(error = %e, "Fatal error");

    return Err(BootError::Fatal(
      e.to_string()
    ));
  }

  Ok(())
}

fn pick_config_path(
  arg1: Option<String>
) -> PathBuf {
  if let Some(p) = arg1 {
    return PathBuf::from(p);
  }

  if let Ok(p) =
    std::env::var("CONFIG_PATH")
    && !p.trim().is_empty()
  {
    return PathBuf::from(p);
  }

  // Prefer repo-local res/ config; fall
  // back to old resources path for
  // compatibility.
  let candidates = [
    PathBuf::from(
      "crates/fetcher/res/config.toml"
    ),
    PathBuf::from(
      "fetcher/res/config.toml"
    ),
    PathBuf::from("res/config.toml"),
    PathBuf::from(
      "src/main/resources/config/\
       config.toml"
    )
  ];

  for p in &candidates {
    if p.exists() {
      return p.clone();
    }
  }

  candidates[0].clone()
}

enum RunMode {
  Scheduler,
  IngestBenchmark {
    feeds_to_insert: usize
  }
}

struct Args {
  config_path: Option<String>,
  mode:        RunMode
}

fn parse_args() -> Args {
  let mut args =
    std::env::args().skip(1);

  let mut config_path = None;

  let mut mode = RunMode::Scheduler;

  while let Some(arg) = args.next() {
    if arg == "--ingest-benchmark" {
      if let Some(n) = args.next() {
        let feeds_to_insert = n
          .parse::<usize>()
          .unwrap_or(0);

        mode =
          RunMode::IngestBenchmark {
            feeds_to_insert
          };
      }
    } else {
      config_path = Some(arg);
    }
  }

  Args {
    config_path,
    mode
  }
}

async fn ingest_feeds<R, I>(
  repo: Arc<R>,
  cfg: Arc<AppConfig>,
  feeds: I
) -> Result<(), BootError>
where
  R: Repo + ?Sized + 'static,
  I: IntoIterator<Item = FeedConfig>
    + Send,
  I::IntoIter: Send
{
  // Large chunks keep transaction
  // overhead low without blowing
  // memory.
  let chunk_size = 10_000;

  let feed_vec: Vec<FeedConfig> =
    feeds.into_iter().collect();

  repo
    .upsert_feeds_bulk(
      feed_vec,
      chunk_size,
      &cfg.timezone
    )
    .await
    .map_err(BootError::Fatal)
}

fn watches_to_feeds(
  watches: &[WatchConfig]
) -> Vec<FeedConfig> {
  watches
    .iter()
    .map(|w| {
      FeedConfig {
        id:                w.id.clone(),
        url:               w
          .url
          .clone(),
        domain:            w
          .domain
          .clone(),
        category:          w
          .category
          .clone(),
        base_poll_seconds: w
          .base_poll_seconds,
        provenance:        w
          .provenance
          .clone()
          .or_else(|| {
            Some(
              "ad-hoc-watch"
                .to_string()
            )
          }),
        tags:              w
          .tags
          .clone(),
        language:          w
          .language
          .clone(),
        content_type:      w
          .content_type
          .clone()
          .or_else(|| {
            Some(
              "text/html".to_string()
            )
          })
      }
    })
    .collect()
}

fn benchmark_feed_stream(
  count: usize,
  base_poll_seconds: u64,
  category: String
) -> impl Iterator<Item = FeedConfig> {
  (0..count).map(move |i| {
    FeedConfig {
      id: format!(
        "benchmark-feed-{i}"
      ),
      url: format!(
        "https://example.org/feed/{i}.xml"
      ),
      domain: "example.org".to_string(),
      category: category.clone(),
      base_poll_seconds,
      provenance: Some(
        "benchmark".to_string(),
      ),
      tags: None,
      language: Some(
        "en".to_string(),
      ),
      content_type: Some(
        "application/rss+xml"
          .to_string(),
      ),
    }
  })
}
