//! Deterministic, offline acceptance
//! exercises for scheduler category
//! isolation and bounded due-feed
//! batching.

use std::collections::{
  HashMap,
  HashSet
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{
  AtomicUsize,
  Ordering
};
use std::time::{
  Duration,
  Instant
};

use chrono_tz::Tz;
use tokio::sync::{
  Mutex,
  Notify
};

use super::Scheduler;
use super::concurrency::ConcurrencyGuards;
use super::processing::run_tick;
use crate::app::context::AppContext;
use crate::domain::link_state::{
  LinkPhase,
  LinkState
};
use crate::domain::model::{
  AppConfig,
  AppMode,
  DomainConfig,
  ErrorKind,
  FeedConfig,
  GetResult,
  HeadResult,
  MetricsConfig,
  PostgresConfig,
  SqlDialect
};
use crate::feed::parser::ParsedFeed;
use crate::ports::clock::Clock;
use crate::ports::http::Http;
use crate::ports::random::RandomSource;
use crate::ports::repo::{
  Repo,
  StateRow
};

struct FakeRepo {
  feeds:           Vec<FeedConfig>,
  completed: Mutex<HashSet<String>>,
  phases:
    Mutex<HashMap<String, LinkPhase>>,
  page_sizes:      Mutex<Vec<usize>>,
  blocked_entered: Arc<Notify>,
  blocked_release: Arc<Notify>,
  healthy_seen:    Arc<Notify>,
  block_slow:      bool,
  fail_slow:       bool
}

impl FakeRepo {
  fn new(
    feeds: Vec<FeedConfig>,
    block_slow: bool,
    fail_slow: bool
  ) -> Self {
    Self {
      feeds,
      completed: Mutex::new(
        HashSet::new()
      ),
      phases: Mutex::new(HashMap::new()),
      page_sizes: Mutex::new(Vec::new()),
      blocked_entered: Arc::new(
        Notify::new()
      ),
      blocked_release: Arc::new(
        Notify::new()
      ),
      healthy_seen: Arc::new(
        Notify::new()
      ),
      block_slow,
      fail_slow
    }
  }
}

#[async_trait::async_trait]
#[allow(clippy::too_many_arguments)]
impl Repo for FakeRepo {
  async fn migrate(
    &self,
    _zone: &Tz,
    _default_poll_seconds: u64
  ) -> Result<(), String> {
    Ok(())
  }

  async fn upsert_feeds_bulk(
    &self,
    _feeds: Vec<FeedConfig>,
    _chunk_size: usize,
    _zone: &Tz
  ) -> Result<(), String> {
    Ok(())
  }

  async fn upsert_categories(
    &self,
    _categories: Vec<String>,
    _zone: &Tz
  ) -> Result<(), String> {
    Ok(())
  }

  async fn latest_state(
    &self,
    _feed_id: &str
  ) -> Result<Option<StateRow>, String>
  {
    Ok(None)
  }

  async fn due_feeds_for_category(
    &self,
    category: &str,
    _now_ms: i64,
    limit: i64
  ) -> Result<Vec<FeedConfig>, String>
  {
    if category == "slow" {
      self.blocked_entered.notify_one();
      if self.fail_slow {
        return Err(
          "synthetic due-query error"
            .into()
        );
      }
      if self.block_slow {
        self
          .blocked_release
          .notified()
          .await;
      }
    }

    if category == "healthy" {
      self.healthy_seen.notify_one();
    }

    let completed =
      self.completed.lock().await;
    let due: Vec<_> = self
      .feeds
      .iter()
      .filter(|feed| {
        feed.category == category
          && !completed
            .contains(&feed.id)
      })
      .take(limit.max(0) as usize)
      .cloned()
      .collect();
    drop(completed);

    self
      .page_sizes
      .lock()
      .await
      .push(due.len());
    Ok(due)
  }

  async fn insert_state(
    &self,
    state: &LinkState,
    _recorded_at_ms: i64,
    _zone: &Tz,
    _record_history: bool
  ) -> Result<(), String> {
    self
      .completed
      .lock()
      .await
      .insert(state.feed_id.clone());
    self.phases.lock().await.insert(
      state.feed_id.clone(),
      state.phase
    );
    Ok(())
  }

  async fn insert_event(
    &self,
    _feed_id: &str,
    _method: &str,
    _status: Option<i64>,
    _error_kind: Option<ErrorKind>,
    _latency_ms: Option<i64>,
    _backoff_index: i64,
    _scheduled_next_action_at_ms: i64,
    _debug: Option<&str>,
    _zone: &Tz
  ) -> Result<(), String> {
    Ok(())
  }

  async fn insert_payload_with_items(
    &self,
    _feed_id: &str,
    _fetched_at_ms: i64,
    _etag: Option<&str>,
    _last_modified_ms: Option<i64>,
    _content_hash: Option<&str>,
    _parsed: &ParsedFeed,
    _zone: &Tz
  ) -> Result<(), String> {
    Ok(())
  }

  async fn mark_feed_error(
    &self,
    _feed_id: &str,
    _error_kind: Option<ErrorKind>,
    _status: Option<i64>,
    _error_count: i64,
    _observed_at_ms: i64,
    _zone: &Tz
  ) -> Result<(), String> {
    Ok(())
  }

  async fn latest_cookie_header(
    &self,
    _feed_id: &str
  ) -> Result<Option<String>, String>
  {
    Ok(None)
  }

  async fn upsert_cookie_header(
    &self,
    _feed_id: &str,
    _cookie_header: &str,
    _observed_at_ms: i64,
    _zone: &Tz
  ) -> Result<(), String> {
    Ok(())
  }
}

#[derive(Default)]
struct FakeHttp {
  inflight:          AtomicUsize,
  peak:              AtomicUsize,
  block_slow_http:   bool,
  fail_slow_http:    bool,
  slow_http_entered: Notify,
  slow_http_release: Notify,
  healthy_http_seen: Notify
}

#[async_trait::async_trait]
impl Http for FakeHttp {
  async fn head(
    &self,
    _url: &str,
    _cookie_header: Option<&str>,
    _extra_headers: Option<
      &HashMap<String, String>
    >
  ) -> HeadResult {
    panic!(
      "fixture feeds start with GET"
    )
  }

  async fn get(
    &self,
    url: &str,
    _cookie_header: Option<&str>,
    _extra_headers: Option<
      &HashMap<String, String>
    >
  ) -> GetResult {
    let concurrent = self
      .inflight
      .fetch_add(1, Ordering::SeqCst)
      + 1;
    self.peak.fetch_max(
      concurrent,
      Ordering::SeqCst
    );
    if self.block_slow_http
      && url.ends_with("/slow")
    {
      self
        .slow_http_entered
        .notify_one();
      self
        .slow_http_release
        .notified()
        .await;
    }
    if url.ends_with("/healthy") {
      self
        .healthy_http_seen
        .notify_one();
    }
    tokio::time::sleep(
      Duration::from_millis(1)
    )
    .await;
    self
      .inflight
      .fetch_sub(1, Ordering::SeqCst);
    let failed = self.fail_slow_http
      && url.ends_with("/slow");
    GetResult {
      status:             if failed {
        None
      } else {
        Some(200)
      },
      body:               None,
      etag:               None,
      last_modified:      None,
      error:              if failed {
        Some(ErrorKind::Timeout)
      } else {
        None
      },
      latency_ms:         1,
      set_cookie_headers: Vec::new()
    }
  }
}

struct FakeClock;

#[async_trait::async_trait]
impl Clock for FakeClock {
  async fn now_epoch_ms(&self) -> i64 {
    0
  }
}

struct FakeRandom;

#[async_trait::async_trait]
impl RandomSource for FakeRandom {
  async fn next_f64(&self) -> f64 {
    0.5
  }
}

type TestContext = AppContext<
  FakeRepo,
  FakeHttp,
  FakeClock,
  FakeRandom
>;

fn fixture(
  repo: Arc<FakeRepo>,
  http: Arc<FakeHttp>
) -> TestContext {
  let mut domains = HashMap::new();
  domains.insert(
    "example.test".to_string(),
    DomainConfig {
      max_concurrent_requests: 4
    }
  );
  let cfg = AppConfig {
    db_dialect: SqlDialect::Sqlite,
    sqlite_path: PathBuf::from(
      ":memory:"
    ),
    postgres: PostgresConfig {
      user:     "test".into(),
      password: "not-used".into(),
      host:     "127.0.0.1".into(),
      port:     5432,
      database: "test".into(),
      schema:   "public".into()
    },
    default_poll_seconds: 60,
    max_poll_seconds: 3600,
    error_backoff_base_seconds: 2,
    max_error_backoff_seconds: 60,
    max_consecutive_errors: 5,
    immediate_error_statuses: Vec::new(
    ),
    jitter_fraction: 0.0,
    global_max_concurrent_requests:
      Some(4),
    user_agent:
      "pulsewire-offline-test".into(),
    log_level: "error".into(),
    log_file_enabled: false,
    log_file_level: "error".into(),
    log_file_directory: PathBuf::from(
      "/tmp"
    ),
    log_file_name: "scheduler-test.log"
      .into(),
    log_file_rotation: "never".into(),
    log_tick_warn_seconds: 0,
    log_feed_timing_enabled: false,
    log_feed_timing_domains: Vec::new(),
    log_feed_timing_warn_ms: 0,
    log_feed_timing_log_all: false,
    metrics: MetricsConfig {
      enabled: false,
      bind:    "127.0.0.1:0".into()
    },
    mode: AppMode::Dev,
    timezone: chrono_tz::UTC,
    domains,
    state_history_sample_rate: 1.0
  };

  AppContext {
    cfg: Arc::new(cfg),
    repo,
    http,
    clock: Arc::new(FakeClock),
    rng: Arc::new(FakeRandom),
    watches_by_id: Arc::new(
      HashMap::new()
    ),
    cookie_header_by_id: Arc::new(
      HashMap::new()
    ),
    extra_headers_by_id: Arc::new(
      HashMap::new()
    )
  }
}

#[tokio::test]
async fn slow_due_query_does_not_block_healthy_category()
 {
  let repo = Arc::new(FakeRepo::new(
    Vec::new(),
    true,
    false
  ));
  let slow =
    repo.blocked_entered.clone();
  let healthy =
    repo.healthy_seen.clone();
  let ctx = fixture(
    repo,
    Arc::new(FakeHttp::default())
  );
  let worker = tokio::spawn(
    async move {
      Scheduler::run_forever_by_category(
      ctx,
      vec!["slow".into(), "healthy".into()]
    ).await
    }
  );

  let observed = tokio::time::timeout(
    Duration::from_secs(2),
    async {
      slow.notified().await;
      healthy.notified().await;
    }
  )
  .await;
  worker.abort();
  let _ = worker.await;
  assert!(
    observed.is_ok(),
    "healthy category must advance \
     while slow query is blocked"
  );
}

#[tokio::test]
async fn failing_category_retries_without_stalling_healthy()
 {
  let repo = Arc::new(FakeRepo::new(
    Vec::new(),
    false,
    true
  ));
  let healthy =
    repo.healthy_seen.clone();
  let ctx = fixture(
    repo,
    Arc::new(FakeHttp::default())
  );
  let worker = tokio::spawn(
    async move {
      Scheduler::run_forever_by_category(
      ctx,
      vec!["slow".into(), "healthy".into()]
    ).await
    }
  );

  let observed = tokio::time::timeout(
    Duration::from_secs(2),
    healthy.notified()
  )
  .await;
  worker.abort();
  let _ = worker.await;
  assert!(
    observed.is_ok(),
    "a failing category must not \
     delay independent scheduling"
  );
}

#[tokio::test]
async fn backlog_pages_are_bounded_and_drain_without_repeats()
 {
  let feeds: Vec<FeedConfig> = (0
    ..1101)
    .map(|i| {
      FeedConfig {
        id:                format!(
          "feed-{i:04}"
        ),
        url:               format!(
          "https://example.test/{i}"
        ),
        domain:
          "example.test".into(),
        category:          "bulk"
          .into(),
        base_poll_seconds: 60,
        provenance:        None,
        tags:              None,
        language:          None,
        content_type:      None,
        cookie_path:       None,
        headers_path:      None,
        headers:           None
      }
    })
    .collect();
  let repo = Arc::new(FakeRepo::new(
    feeds, false, false
  ));
  let http =
    Arc::new(FakeHttp::default());
  let ctx =
    fixture(repo.clone(), http.clone());
  let guards = ConcurrencyGuards::new(
    ctx.cfg.clone()
  );

  run_tick(
    &ctx,
    &guards,
    Instant::now(),
    "bulk",
    4
  )
  .await
  .expect("first due batch");
  assert_eq!(
    repo.completed.lock().await.len(),
    1000
  );

  run_tick(
    &ctx,
    &guards,
    Instant::now(),
    "bulk",
    4
  )
  .await
  .expect("second due batch");
  assert_eq!(
    repo.completed.lock().await.len(),
    1101
  );
  assert_eq!(
    *repo.page_sizes.lock().await,
    vec![1000, 101]
  );
  assert!(
    http.peak.load(Ordering::SeqCst)
      <= 4
  );
  assert_eq!(
    http
      .inflight
      .load(Ordering::SeqCst),
    0
  );
}

fn mock_feed(
  id: &str,
  category: &str,
  domain: &str
) -> FeedConfig {
  FeedConfig {
    id:                id.into(),
    url:               format!(
      "https://{domain}/{id}"
    ),
    domain:            domain.into(),
    category:          category.into(),
    base_poll_seconds: 60,
    provenance:        None,
    tags:              None,
    language:          None,
    content_type:      None,
    cookie_path:       None,
    headers_path:      None,
    headers:           None
  }
}

#[tokio::test]
async fn slow_http_feed_does_not_starve_another_category()
 {
  let repo = Arc::new(FakeRepo::new(
    vec![
      mock_feed(
        "slow",
        "slow",
        "slow.test"
      ),
      mock_feed(
        "healthy",
        "healthy",
        "healthy.test"
      ),
    ],
    false,
    false
  ));
  let http = Arc::new(FakeHttp {
    block_slow_http: true,
    ..Default::default()
  });
  let ctx = fixture(repo, http.clone());
  let worker = tokio::spawn(
    async move {
      Scheduler::run_forever_by_category(
      ctx,
      vec!["slow".into(), "healthy".into()]
    ).await
    }
  );

  let advanced = tokio::time::timeout(
    Duration::from_secs(2),
    async {
      http
        .slow_http_entered
        .notified()
        .await;
      http
        .healthy_http_seen
        .notified()
        .await;
    }
  )
  .await;
  http.slow_http_release.notify_one();
  worker.abort();
  let _ = worker.await;

  assert!(
    advanced.is_ok(),
    "slow HTTP feed must not stall \
     unrelated category"
  );
}

#[tokio::test]
async fn failed_http_feed_does_not_cascade_and_enters_error_backoff()
 {
  let repo = Arc::new(FakeRepo::new(
    vec![
      mock_feed(
        "slow",
        "slow",
        "slow.test"
      ),
      mock_feed(
        "healthy",
        "healthy",
        "healthy.test"
      ),
    ],
    false,
    false
  ));
  let http = Arc::new(FakeHttp {
    fail_slow_http: true,
    ..Default::default()
  });
  let ctx = fixture(repo.clone(), http);
  let worker = tokio::spawn(
    async move {
      Scheduler::run_forever_by_category(
      ctx,
      vec!["slow".into(), "healthy".into()]
    ).await
    }
  );

  let advanced = tokio::time::timeout(
    Duration::from_secs(2),
    async {
      loop {
        let phases =
          repo.phases.lock().await;
        let slow_backoff = phases
          .get("slow")
          == Some(
            &LinkPhase::ErrorBackoff
          );
        let healthy_advanced = phases
          .get("healthy")
          == Some(
            &LinkPhase::NeedsHead
          );
        drop(phases);
        if slow_backoff
          && healthy_advanced
        {
          break;
        }
        tokio::time::sleep(
          Duration::from_millis(5)
        )
        .await;
      }
    }
  )
  .await;

  worker.abort();
  let _ = worker.await;
  assert!(
    advanced.is_ok(),
    "failed HTTP request must enter \
     backoff without blocking healthy \
     category"
  );
}

#[tokio::test]
async fn sustained_multicategory_queue_is_bounded()
 {
  let categories =
    ["news", "government", "journals"];
  let feeds_per_category = 1075usize;

  let mut feeds = Vec::new();
  for category in categories {
    let domain =
      format!("{category}.test");
    for n in 0..feeds_per_category {
      let id =
        format!("{category}-{n:04}");
      feeds.push(mock_feed(
        &id, category, &domain
      ));
    }
  }

  let repo = Arc::new(FakeRepo::new(
    feeds, false, false
  ));
  let http =
    Arc::new(FakeHttp::default());
  let ctx =
    fixture(repo.clone(), http.clone());
  let guards = ConcurrencyGuards::new(
    ctx.cfg.clone()
  );

  let started = Instant::now();
  tokio::time::timeout(
    Duration::from_secs(30),
    async {
      // Each category has >1,000 due
      // feeds: two complete sweeps
      // drain them, followed by an
      // empty sweep to prove no
      // reprocessing.
      for sweep in 0..3 {
        let ticks = categories
          .iter()
          .map(|category| {
            run_tick(
              &ctx,
              &guards,
              Instant::now(),
              category,
              2
            )
          });
        for result in
          futures::future::join_all(
            ticks
          )
          .await
        {
          result.expect(
            "synthetic category tick"
          );
        }

        let completed =
          repo.completed.lock().await;
        for category in categories {
          let count = completed
            .iter()
            .filter(|id| {
              id.starts_with(category)
            })
            .count();
          let expected = if sweep == 0 {
            1000
          } else {
            feeds_per_category
          };
          assert_eq!(
            count, expected,
            "all categories must make \
             progress on each sweep"
          );
        }
      }
    }
  )
  .await
  .expect(
    "synthetic load exceeded bounded \
     duration"
  );

  let completed =
    repo.completed.lock().await;
  assert_eq!(
    completed.len(),
    categories.len()
      * feeds_per_category
  );
  drop(completed);

  let mut page_sizes = repo
    .page_sizes
    .lock()
    .await
    .clone();
  page_sizes.sort_unstable();
  assert_eq!(
    page_sizes,
    vec![
      0, 0, 0, 75, 75, 75, 1000, 1000,
      1000
    ],
    "queue batches must stay capped \
     and drain without repeats"
  );
  let peak =
    http.peak.load(Ordering::SeqCst);
  assert!(
    (2..=4).contains(&peak),
    "global concurrency must remain \
     bounded, but multiple categories \
     must progress"
  );
  assert_eq!(
    http
      .inflight
      .load(Ordering::SeqCst),
    0
  );

  eprintln!(
    "PW-031 isolated synthetic load: \
     feeds={}, categories={}, \
     rounds=3, peak_http={}, \
     elapsed_ms={}",
    categories.len()
      * feeds_per_category,
    categories.len(),
    peak,
    started.elapsed().as_millis()
  );
}
