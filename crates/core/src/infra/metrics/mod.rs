//! Minimal Prometheus text endpoint
//! for internal runtime stats.

mod histogram;
mod render;
mod server;

use std::collections::HashMap;
use std::sync::atomic::{
  AtomicU64,
  Ordering
};
use std::sync::{
  Arc,
  Mutex,
  OnceLock
};
use std::time::{
  SystemTime,
  UNIX_EPOCH
};

use histogram::{
  Histogram,
  record_histogram
};

use crate::domain::model::{
  CategoryConfig,
  MetricsConfig
};

const LATENCY_BUCKETS_MS: [u64; 9] = [
  25, 50, 100, 250, 500, 1000, 2000,
  5000, 10000
];

const DB_BUCKETS_MS: [u64; 9] =
  [1, 2, 5, 10, 25, 50, 100, 250, 500];

#[derive(Debug)]
pub struct Metrics {
  start_time_seconds: u64,
  ticks_by_category:
    Mutex<HashMap<String, u64>>,
  due_feeds_by_category:
    Mutex<HashMap<String, u64>>,
  due_feeds_current_by_category:
    Mutex<HashMap<String, u64>>,
  head_ok: AtomicU64,
  head_err: AtomicU64,
  get_ok: AtomicU64,
  get_err: AtomicU64,
  inflight_actions: AtomicU64,
  status_counts:
    Mutex<HashMap<String, u64>>,
  http_latency:
    Mutex<HashMap<String, Histogram>>,
  db_timings:
    Mutex<HashMap<String, Histogram>>
}

static METRICS: OnceLock<Arc<Metrics>> =
  OnceLock::new();

impl Metrics {
  fn new(
    categories: &[CategoryConfig]
  ) -> Self {
    let mut ticks = HashMap::new();
    let mut due = HashMap::new();
    let mut due_current =
      HashMap::new();

    for c in categories {
      ticks.insert(c.name.clone(), 0);
      due.insert(c.name.clone(), 0);
      due_current
        .insert(c.name.clone(), 0);
    }

    let start_time_seconds =
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Self {
      start_time_seconds,
      ticks_by_category: Mutex::new(
        ticks
      ),
      due_feeds_by_category: Mutex::new(
        due
      ),
      due_feeds_current_by_category:
        Mutex::new(due_current),
      head_ok: AtomicU64::new(0),
      head_err: AtomicU64::new(0),
      get_ok: AtomicU64::new(0),
      get_err: AtomicU64::new(0),
      inflight_actions: AtomicU64::new(
        0
      ),
      status_counts: Mutex::new(
        HashMap::new()
      ),
      http_latency: Mutex::new(
        HashMap::new()
      ),
      db_timings: Mutex::new(
        HashMap::new()
      )
    }
  }

  fn render(&self) -> String {
    render::render(self)
  }
}

pub async fn init(
  cfg: &MetricsConfig,
  categories: &[CategoryConfig]
) -> Result<(), String> {
  if !cfg.enabled {
    return Ok(());
  }

  let metrics =
    Arc::new(Metrics::new(categories));

  METRICS
    .set(metrics.clone())
    .map_err(|_| {
      "metrics already initialized"
        .to_string()
    })?;

  server::spawn(&cfg.bind, metrics)
    .await
}

pub fn record_tick(
  category: &str,
  due_count: u64
) {
  let Some(metrics) = METRICS.get()
  else {
    return;
  };

  if let Ok(mut ticks) =
    metrics.ticks_by_category.lock()
  {
    *ticks
      .entry(category.to_string())
      .or_insert(0) += 1;
  }

  if let Ok(mut due) =
    metrics.due_feeds_by_category.lock()
  {
    *due
      .entry(category.to_string())
      .or_insert(0) += due_count;
  }

  if let Ok(mut due_current) = metrics
    .due_feeds_current_by_category
    .lock()
  {
    due_current.insert(
      category.to_string(),
      due_count
    );
  }
}

pub fn record_http_result(
  action: &str,
  domain: &str,
  status: Option<u16>,
  latency_ms: u64,
  ok: bool
) {
  let Some(metrics) = METRICS.get()
  else {
    return;
  };

  match (action, ok) {
    | ("head", true) => {
      metrics.head_ok.fetch_add(
        1,
        Ordering::Relaxed
      );
    }
    | ("head", false) => {
      metrics.head_err.fetch_add(
        1,
        Ordering::Relaxed
      );
    }
    | ("get", true) => {
      metrics.get_ok.fetch_add(
        1,
        Ordering::Relaxed
      );
    }
    | ("get", false) => {
      metrics.get_err.fetch_add(
        1,
        Ordering::Relaxed
      );
    }
    | _ => {}
  }

  let status_label = status
    .map(|s| s.to_string())
    .unwrap_or_else(|| {
      "error".to_string()
    });

  if let Ok(mut statuses) =
    metrics.status_counts.lock()
  {
    let key = format!(
      "{action}|{status_label}"
    );

    *statuses
      .entry(key)
      .or_insert(0) += 1;
  }

  if let Ok(mut latency) =
    metrics.http_latency.lock()
  {
    let key =
      format!("{action}|{domain}");

    record_histogram(
      latency
        .entry(key)
        .or_insert_with(|| {
          Histogram::new(
            LATENCY_BUCKETS_MS.len()
          )
        }),
      latency_ms,
      &LATENCY_BUCKETS_MS
    );
  }
}

pub fn record_db_time(
  query: &str,
  elapsed_ms: u64
) {
  let Some(metrics) = METRICS.get()
  else {
    return;
  };

  if let Ok(mut db) =
    metrics.db_timings.lock()
  {
    let key = query.to_string();

    record_histogram(
      db.entry(key).or_insert_with(
        || {
          Histogram::new(
            DB_BUCKETS_MS.len()
          )
        }
      ),
      elapsed_ms,
      &DB_BUCKETS_MS
    );
  }
}

pub fn record_inflight_start()
-> InFlightGuard {
  let Some(metrics) = METRICS.get()
  else {
    return InFlightGuard {
      enabled: false
    };
  };

  metrics
    .inflight_actions
    .fetch_add(1, Ordering::Relaxed);

  InFlightGuard {
    enabled: true
  }
}

pub struct InFlightGuard {
  enabled: bool
}

impl Drop for InFlightGuard {
  fn drop(&mut self) {
    if !self.enabled {
      return;
    }

    if let Some(metrics) = METRICS.get()
    {
      metrics
        .inflight_actions
        .fetch_sub(
          1,
          Ordering::Relaxed
        );
    }
  }
}
