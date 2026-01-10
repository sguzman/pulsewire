//! Minimal Prometheus text endpoint for internal runtime stats.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::domain::model::{CategoryConfig, MetricsConfig};

const LATENCY_BUCKETS_MS: [u64; 9] = [25, 50, 100, 250, 500, 1000, 2000, 5000, 10000];
const DB_BUCKETS_MS: [u64; 9] = [1, 2, 5, 10, 25, 50, 100, 250, 500];

#[derive(Debug)]
pub struct Metrics {
    start_time_seconds: u64,
    ticks_by_category: Mutex<HashMap<String, u64>>,
    due_feeds_by_category: Mutex<HashMap<String, u64>>,
    due_feeds_current_by_category: Mutex<HashMap<String, u64>>,
    head_ok: AtomicU64,
    head_err: AtomicU64,
    get_ok: AtomicU64,
    get_err: AtomicU64,
    inflight_actions: AtomicU64,
    status_counts: Mutex<HashMap<String, u64>>,
    http_latency: Mutex<HashMap<String, Histogram>>,
    db_timings: Mutex<HashMap<String, Histogram>>,
}

static METRICS: OnceLock<Arc<Metrics>> = OnceLock::new();

impl Metrics {
    fn new(categories: &[CategoryConfig]) -> Self {
        let mut ticks = HashMap::new();
        let mut due = HashMap::new();
        let mut due_current = HashMap::new();
        for c in categories {
            ticks.insert(c.name.clone(), 0);
            due.insert(c.name.clone(), 0);
            due_current.insert(c.name.clone(), 0);
        }

        let start_time_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            start_time_seconds,
            ticks_by_category: Mutex::new(ticks),
            due_feeds_by_category: Mutex::new(due),
            due_feeds_current_by_category: Mutex::new(due_current),
            head_ok: AtomicU64::new(0),
            head_err: AtomicU64::new(0),
            get_ok: AtomicU64::new(0),
            get_err: AtomicU64::new(0),
            inflight_actions: AtomicU64::new(0),
            status_counts: Mutex::new(HashMap::new()),
            http_latency: Mutex::new(HashMap::new()),
            db_timings: Mutex::new(HashMap::new()),
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# HELP feedrv3_up Service is running.\n");
        out.push_str("# TYPE feedrv3_up gauge\n");
        out.push_str("feedrv3_up 1\n");

        out.push_str("# HELP feedrv3_start_time_seconds Start time in unix seconds.\n");
        out.push_str("# TYPE feedrv3_start_time_seconds gauge\n");
        out.push_str(&format!(
            "feedrv3_start_time_seconds {}\n",
            self.start_time_seconds
        ));

        let ticks = self
            .ticks_by_category
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        out.push_str("# HELP feedrv3_scheduler_ticks_total Scheduler ticks per category.\n");
        out.push_str("# TYPE feedrv3_scheduler_ticks_total counter\n");
        for (category, count) in sorted_map(&ticks) {
            out.push_str(&format!(
                "feedrv3_scheduler_ticks_total{{category=\"{}\"}} {}\n",
                escape_label(&category),
                count
            ));
        }

        let due = self
            .due_feeds_by_category
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        out.push_str("# HELP feedrv3_due_feeds_total Due feeds seen per category.\n");
        out.push_str("# TYPE feedrv3_due_feeds_total counter\n");
        for (category, count) in sorted_map(&due) {
            out.push_str(&format!(
                "feedrv3_due_feeds_total{{category=\"{}\"}} {}\n",
                escape_label(&category),
                count
            ));
        }

        let due_current = self
            .due_feeds_current_by_category
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        out.push_str("# HELP feedrv3_due_feeds Current due feeds per category.\n");
        out.push_str("# TYPE feedrv3_due_feeds gauge\n");
        for (category, count) in sorted_map(&due_current) {
            out.push_str(&format!(
                "feedrv3_due_feeds{{category=\"{}\"}} {}\n",
                escape_label(&category),
                count
            ));
        }

        out.push_str("# HELP feedrv3_inflight_actions In-flight feed actions.\n");
        out.push_str("# TYPE feedrv3_inflight_actions gauge\n");
        out.push_str(&format!(
            "feedrv3_inflight_actions {}\n",
            self.inflight_actions.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP feedrv3_feed_actions_total Feed actions by type and outcome.\n");
        out.push_str("# TYPE feedrv3_feed_actions_total counter\n");
        out.push_str(&format!(
            "feedrv3_feed_actions_total{{action=\"head\",outcome=\"ok\"}} {}\n",
            self.head_ok.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "feedrv3_feed_actions_total{{action=\"head\",outcome=\"err\"}} {}\n",
            self.head_err.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "feedrv3_feed_actions_total{{action=\"get\",outcome=\"ok\"}} {}\n",
            self.get_ok.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "feedrv3_feed_actions_total{{action=\"get\",outcome=\"err\"}} {}\n",
            self.get_err.load(Ordering::Relaxed)
        ));

        let status_counts = self
            .status_counts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        out.push_str("# HELP feedrv3_http_status_total HTTP status counts by action.\n");
        out.push_str("# TYPE feedrv3_http_status_total counter\n");
        for ((action, status), count) in sorted_kv_map(&status_counts) {
            out.push_str(&format!(
                "feedrv3_http_status_total{{action=\"{}\",status=\"{}\"}} {}\n",
                escape_label(&action),
                escape_label(&status),
                count
            ));
        }

        let http_latency = self
            .http_latency
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        out.push_str("# HELP feedrv3_http_latency_ms HTTP latency by action/domain.\n");
        out.push_str("# TYPE feedrv3_http_latency_ms histogram\n");
        for ((action, domain), hist) in sorted_hist_map(&http_latency) {
            emit_histogram(
                &mut out,
                "feedrv3_http_latency_ms",
                &LATENCY_BUCKETS_MS,
                &hist,
                &[("action", &action), ("domain", &domain)],
            );
        }

        let db_timings = self
            .db_timings
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        out.push_str("# HELP feedrv3_db_query_ms Database query timings.\n");
        out.push_str("# TYPE feedrv3_db_query_ms histogram\n");
        for (query, hist) in sorted_hist_map_single(&db_timings) {
            emit_histogram(
                &mut out,
                "feedrv3_db_query_ms",
                &DB_BUCKETS_MS,
                &hist,
                &[("query", &query)],
            );
        }

        out
    }
}

pub async fn init(cfg: &MetricsConfig, categories: &[CategoryConfig]) -> Result<(), String> {
    if !cfg.enabled {
        return Ok(());
    }

    let metrics = Arc::new(Metrics::new(categories));
    METRICS
        .set(metrics.clone())
        .map_err(|_| "metrics already initialized".to_string())?;

    let addr: SocketAddr = cfg
        .bind
        .parse()
        .map_err(|e| format!("invalid metrics bind '{}': {e}", cfg.bind))?;

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("failed to bind metrics server on {}: {e}", cfg.bind))?;

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => continue,
            };
            let metrics = metrics.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let n = match stream.read(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => return,
                };
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");

                let (status, body) = if path == "/metrics" {
                    ("200 OK", metrics.render())
                } else {
                    ("404 Not Found", "not found\n".to_string())
                };

                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });

    Ok(())
}

pub fn record_tick(category: &str, due_count: u64) {
    let Some(metrics) = METRICS.get() else {
        return;
    };

    if let Ok(mut ticks) = metrics.ticks_by_category.lock() {
        *ticks.entry(category.to_string()).or_insert(0) += 1;
    }
    if let Ok(mut due) = metrics.due_feeds_by_category.lock() {
        *due.entry(category.to_string()).or_insert(0) += due_count;
    }
    if let Ok(mut due_current) = metrics.due_feeds_current_by_category.lock() {
        due_current.insert(category.to_string(), due_count);
    }
}

pub fn record_http_result(
    action: &str,
    domain: &str,
    status: Option<u16>,
    latency_ms: u64,
    ok: bool,
) {
    let Some(metrics) = METRICS.get() else {
        return;
    };

    match (action, ok) {
        ("head", true) => {
            metrics.head_ok.fetch_add(1, Ordering::Relaxed);
        }
        ("head", false) => {
            metrics.head_err.fetch_add(1, Ordering::Relaxed);
        }
        ("get", true) => {
            metrics.get_ok.fetch_add(1, Ordering::Relaxed);
        }
        ("get", false) => {
            metrics.get_err.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }

    let status_label = status
        .map(|s| s.to_string())
        .unwrap_or_else(|| "error".to_string());
    if let Ok(mut statuses) = metrics.status_counts.lock() {
        let key = format!("{action}|{status_label}");
        *statuses.entry(key).or_insert(0) += 1;
    }

    if let Ok(mut latency) = metrics.http_latency.lock() {
        let key = format!("{action}|{domain}");
        record_histogram(
            latency
                .entry(key)
                .or_insert_with(|| Histogram::new(LATENCY_BUCKETS_MS.len())),
            latency_ms,
            &LATENCY_BUCKETS_MS,
        );
    }
}

pub fn record_db_time(query: &str, elapsed_ms: u64) {
    let Some(metrics) = METRICS.get() else {
        return;
    };
    if let Ok(mut db) = metrics.db_timings.lock() {
        let key = query.to_string();
        record_histogram(
            db.entry(key)
                .or_insert_with(|| Histogram::new(DB_BUCKETS_MS.len())),
            elapsed_ms,
            &DB_BUCKETS_MS,
        );
    }
}

pub fn record_inflight_start() -> InFlightGuard {
    let Some(metrics) = METRICS.get() else {
        return InFlightGuard { enabled: false };
    };
    metrics.inflight_actions.fetch_add(1, Ordering::Relaxed);
    InFlightGuard { enabled: true }
}

fn escape_label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\"', "\\\"")
}

fn sorted_map(map: &HashMap<String, u64>) -> Vec<(String, u64)> {
    let mut items: Vec<(String, u64)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
}

fn sorted_kv_map(map: &HashMap<String, u64>) -> Vec<((String, String), u64)> {
    let mut items: Vec<((String, String), u64)> = map
        .iter()
        .filter_map(|(k, v)| {
            let mut parts = k.splitn(2, '|');
            let action = parts.next()?.to_string();
            let status = parts.next()?.to_string();
            Some(((action, status), *v))
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
}

fn sorted_hist_map(
    map: &HashMap<String, Histogram>,
) -> Vec<((String, String), Histogram)> {
    let mut items: Vec<((String, String), Histogram)> = map
        .iter()
        .filter_map(|(k, v)| {
            let mut parts = k.splitn(2, '|');
            let action = parts.next()?.to_string();
            let domain = parts.next()?.to_string();
            Some(((action, domain), v.clone()))
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
}

fn sorted_hist_map_single(
    map: &HashMap<String, Histogram>,
) -> Vec<(String, Histogram)> {
    let mut items: Vec<(String, Histogram)> =
        map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
}

fn emit_histogram(
    out: &mut String,
    name: &str,
    buckets: &[u64],
    hist: &Histogram,
    labels: &[(&str, &str)],
) {
    let mut cumulative = 0u64;
    for (idx, upper) in buckets.iter().enumerate() {
        cumulative += hist.buckets.get(idx).copied().unwrap_or(0);
        out.push_str(&format!(
            "{name}_bucket{} {}\\n",
            format_labels(labels, Some(&upper.to_string())),
            cumulative
        ));
    }
    cumulative += hist.buckets.get(buckets.len()).copied().unwrap_or(0);
    out.push_str(&format!(
        "{name}_bucket{} {}\\n",
        format_labels(labels, Some("+Inf")),
        cumulative
    ));
    out.push_str(&format!(
        "{name}_sum{} {}\\n",
        format_labels(labels, None),
        hist.sum
    ));
    out.push_str(&format!(
        "{name}_count{} {}\\n",
        format_labels(labels, None),
        hist.count
    ));
}

fn format_labels(labels: &[(&str, &str)], le: Option<&str>) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for (k, v) in labels {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape_label(v));
        out.push('"');
    }
    if let Some(le) = le {
        if !first {
            out.push(',');
        }
        out.push_str("le=\"");
        out.push_str(le);
        out.push('"');
    }
    out.push('}');
    out
}

#[derive(Debug, Clone)]
struct Histogram {
    buckets: Vec<u64>,
    sum: u64,
    count: u64,
}

impl Histogram {
    fn new(bucket_len: usize) -> Self {
        Self {
            buckets: vec![0; bucket_len + 1],
            sum: 0,
            count: 0,
        }
    }
}

fn record_histogram(hist: &mut Histogram, value_ms: u64, buckets: &[u64]) {
    hist.sum += value_ms;
    hist.count += 1;
    let mut idx = 0;
    while idx < buckets.len() {
        if value_ms <= buckets[idx] {
            hist.buckets[idx] += 1;
            return;
        }
        idx += 1;
    }
    hist.buckets[idx] += 1;
}

pub struct InFlightGuard {
    enabled: bool,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        if let Some(metrics) = METRICS.get() {
            metrics.inflight_actions.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
