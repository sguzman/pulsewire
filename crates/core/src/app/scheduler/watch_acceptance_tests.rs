//! End-to-end HTML-watch acceptance against
//! a disposable SQLite database. Exercises
//! actual scheduler GET, parser fallback,
//! payload transactions and restart state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::actions::do_get;
use super::concurrency::ConcurrencyGuards;
use crate::domain::link_state::LinkState;
use crate::domain::model::{
  AppConfig,
  AppMode,
  DomainConfig,
  FeedConfig,
  GetResult,
  HeadResult,
  MetricsConfig,
  PostgresConfig,
  SqlDialect,
  WatchCheckMethod,
  WatchConfig,
  WatchDetector,
  WatchEmitMode,
  WatchItemIdentity
};
use crate::infra::sqlite_repo::SqliteRepo;
use crate::ports::http::Http;
use crate::ports::repo::Repo;

struct WatchHttp {
  bytes: Vec<u8>,
  status: u16
}

#[async_trait::async_trait]
impl Http for WatchHttp {
  async fn head(
    &self,
    _url: &str,
    _cookie_header: Option<&str>,
    _extra_headers: Option<&HashMap<String, String>>
  ) -> HeadResult {
    panic!("GET-only HTML watch")
  }

  async fn get(
    &self,
    _url: &str,
    _cookie_header: Option<&str>,
    _extra_headers: Option<&HashMap<String, String>>
  ) -> GetResult {
    GetResult {
      status: Some(self.status),
      body: Some(self.bytes.clone()),
      etag: None,
      last_modified: None,
      error: None,
      latency_ms: 1,
      set_cookie_headers: Vec::new()
    }
  }
}

fn watch_test_config(db_file: PathBuf) -> Arc<AppConfig> {
  let mut domains = HashMap::new();
  domains.insert(
    "example.test".into(),
    DomainConfig {
      max_concurrent_requests: 1
    }
  );

  Arc::new(AppConfig {
    db_dialect: SqlDialect::Sqlite,
    sqlite_path: db_file,
    postgres: PostgresConfig {
      user: "unused".into(),
      password: "unused".into(),
      host: "127.0.0.1".into(),
      port: 5432,
      database: "unused".into(),
      schema: "public".into()
    },
    default_poll_seconds: 60,
    max_poll_seconds: 3600,
    error_backoff_base_seconds: 2,
    max_error_backoff_seconds: 60,
    max_consecutive_errors: 5,
    immediate_error_statuses: Vec::new(),
    jitter_fraction: 0.0,
    global_max_concurrent_requests: Some(1),
    user_agent: "pulsewire-watch-acceptance".into(),
    log_level: "error".into(),
    log_file_enabled: false,
    log_file_level: "error".into(),
    log_file_directory: PathBuf::from("/tmp"),
    log_file_name: "watch-acceptance.log".into(),
    log_file_rotation: "never".into(),
    log_tick_warn_seconds: 0,
    log_feed_timing_enabled: false,
    log_feed_timing_domains: Vec::new(),
    log_feed_timing_warn_ms: 0,
    log_feed_timing_log_all: false,
    metrics: MetricsConfig {
      enabled: false,
      bind: "127.0.0.1:0".into()
    },
    mode: AppMode::Prod,
    timezone: chrono_tz::UTC,
    domains,
    state_history_sample_rate: 0.0
  })
}

fn html_items(n: usize, noise: usize) -> Vec<u8> {
  let mut html = String::from("<main>");
  for i in 0..n {
    html.push_str(&format!(
      "<a class='story' href='/story/{i}'>Story {i}</a>"
    ));
  }
  html.push_str(&format!(
    "<aside>noise {noise}</aside></main>"
  ));
  html.into_bytes()
}

async fn run_watch_get(
  cfg: &Arc<AppConfig>,
  guards: &ConcurrencyGuards,
  repo: &Arc<SqliteRepo>,
  feed: &FeedConfig,
  watch: &WatchConfig,
  n: usize,
  noise: usize,
  status: u16,
  at_ms: i64
) {
  let client = Arc::new(WatchHttp {
    bytes: html_items(n, noise),
    status
  });
  let state = LinkState::initial(
    feed.id.clone(), 60, 3600, 0.0, at_ms
  );
  do_get(
    cfg.as_ref(), repo, &client, guards,
    feed, Some(watch), None, None,
    state, at_ms, 0.0, false
  )
  .await
  .expect("watch GET completes");
}

#[tokio::test]
async fn item_threshold_survives_restart_and_ignores_failed_gets()
 {
  let temp = tempfile::tempdir()
    .expect("temporary test directory");
  let file = temp.path().join("watch.db");
  let cfg = watch_test_config(file.clone());
  let guards = ConcurrencyGuards::new(cfg.clone());

  let feed = FeedConfig {
    id: "watch-1".into(),
    url: "https://example.test/list".into(),
    domain: "example.test".into(),
    category: "news".into(),
    base_poll_seconds: 60,
    provenance: None,
    tags: None,
    language: None,
    content_type: None,
    cookie_path: None,
    headers_path: None,
    headers: None
  };
  let watch = WatchConfig {
    id: feed.id.clone(),
    url: feed.url.clone(),
    domain: feed.domain.clone(),
    category: feed.category.clone(),
    base_poll_seconds: 60,
    provenance: None,
    tags: None,
    language: None,
    content_type: None,
    cookie_path: None,
    headers_path: None,
    headers: None,
    check_method: WatchCheckMethod::Get,
    fallback_to_get: false,
    detectors: vec![WatchDetector::ContentHash],
    fetch_body_on_change: false,
    max_body_bytes: None,
    max_items_per_fetch: None,
    item_selector: Some("a.story".into()),
    item_identity: Some(WatchItemIdentity::Href),
    item_identity_attr: None,
    title_selector: None,
    link_selector: None,
    summary_selector: None,
    published_selector: None,
    published_format: None,
    include_selectors: None,
    exclude_selectors: None,
    normalize_whitespace: true,
    strip_query_params: true,
    emit_mode: WatchEmitMode::NewItemsOnly,
    emit_title: None,
    min_item_count_change: Some(2)
  };

  let repo = Arc::new(
    SqliteRepo::new(&file).await
      .expect("disposable database")
  );
  repo.migrate(&chrono_tz::UTC, 60)
    .await
    .expect("real schema migration");
  repo.upsert_categories(
    vec!["news".into()], &chrono_tz::UTC
  )
  .await
  .expect("category");
  repo.upsert_feeds_bulk(
    vec![feed.clone()], 1, &chrono_tz::UTC
  )
  .await
  .expect("feed");

  // One item below threshold, then
  // two items: first emission.
  run_watch_get(
    &cfg, &guards, &repo, &feed,
    &watch, 1, 1, 200, 1000
  ).await;
  assert_eq!(
    repo.latest_watch_emitted_count(
      &feed.id
    ).await.unwrap(),
    None
  );
  run_watch_get(
    &cfg, &guards, &repo, &feed,
    &watch, 2, 2, 200, 2000
  ).await;
  assert_eq!(
    repo.latest_watch_emitted_count(
      &feed.id
    ).await.unwrap(),
    Some(2)
  );

  // Error bodies and small deltas
  // must not alter the last-emitted
  // baseline. A later count of four
  // triggers two additional items.
  run_watch_get(
    &cfg, &guards, &repo, &feed,
    &watch, 100, 3, 503, 3000
  ).await;
  run_watch_get(
    &cfg, &guards, &repo, &feed,
    &watch, 3, 4, 200, 4000
  ).await;
  assert_eq!(
    repo.latest_watch_emitted_count(
      &feed.id
    ).await.unwrap(),
    Some(2)
  );
  run_watch_get(
    &cfg, &guards, &repo, &feed,
    &watch, 4, 5, 200, 5000
  ).await;
  assert_eq!(
    repo.latest_watch_emitted_count(
      &feed.id
    ).await.unwrap(),
    Some(4)
  );

  drop(repo);

  // A brand-new repository instance
  // must retain both emitted GUID
  // history and the count baseline.
  let restarted = Arc::new(
    SqliteRepo::new(&file).await
      .expect("reopen watch database")
  );
  restarted.migrate(
    &chrono_tz::UTC, 60
  ).await.expect("idempotent migration");
  assert_eq!(
    restarted.latest_watch_emitted_count(
      &feed.id
    ).await.unwrap(),
    Some(4)
  );

  run_watch_get(
    &cfg, &guards, &restarted, &feed,
    &watch, 5, 6, 200, 6000
  ).await;
  assert_eq!(
    restarted.latest_watch_emitted_count(
      &feed.id
    ).await.unwrap(),
    Some(4)
  );
  run_watch_get(
    &cfg, &guards, &restarted, &feed,
    &watch, 6, 7, 200, 7000
  ).await;
  assert_eq!(
    restarted.latest_watch_emitted_count(
      &feed.id
    ).await.unwrap(),
    Some(6)
  );

  let known = (0..6)
    .map(|i| {
      format!(
        "{}:https://example.test/story/{i}",
        feed.id
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    restarted.known_watch_item_guids(
      &feed.id, &known
    ).await.unwrap().len(),
    6
  );

  let inspector = sqlx::SqlitePool::connect(
    &format!("sqlite://{}", file.display())
  )
  .await
  .expect("independent inspector");
  let row_count: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM feed_items \
     WHERE feed_id = ?1"
  )
  .bind(&feed.id)
  .fetch_one(&inspector)
  .await
  .expect("count actual emitted rows");
  assert_eq!(row_count, 6);
  inspector.close().await;
}
