use std::collections::HashMap;
use std::sync::atomic::Ordering;

use super::{
  DB_BUCKETS_MS,
  Histogram,
  LATENCY_BUCKETS_MS,
  Metrics
};

pub(super) fn render(
  metrics: &Metrics
) -> String {
  let mut out = String::new();

  out.push_str(
    "# HELP feedrv3_up Service is \
     running.\n"
  );

  out.push_str(
    "# TYPE feedrv3_up gauge\n"
  );

  out.push_str("feedrv3_up 1\n");

  out.push_str(
    "# HELP feedrv3_start_time_seconds Start time in unix seconds.\n",
  );

  out.push_str(
    "# TYPE feedrv3_start_time_seconds gauge\n",
  );

  out.push_str(&format!(
    "feedrv3_start_time_seconds {}\n",
    metrics.start_time_seconds
  ));

  let ticks = metrics
    .ticks_by_category
    .lock()
    .unwrap_or_else(|e| e.into_inner());

  out.push_str(
    "# HELP feedrv3_scheduler_ticks_total Scheduler ticks per category.\n",
  );

  out.push_str(
    "# TYPE feedrv3_scheduler_ticks_total counter\n",
  );

  for (category, count) in
    sorted_map(&ticks)
  {
    out.push_str(&format!(
      "feedrv3_scheduler_ticks_total{{category=\"{}\"}} {}\n",
      escape_label(&category),
      count
    ));
  }

  let due = metrics
    .due_feeds_by_category
    .lock()
    .unwrap_or_else(|e| e.into_inner());

  out.push_str(
    "# HELP feedrv3_due_feeds_total \
     \\\n     Due feeds seen per \
     category.\n"
  );

  out.push_str(
    "# TYPE feedrv3_due_feeds_total \\
     counter\n"
  );

  for (category, count) in
    sorted_map(&due)
  {
    out.push_str(&format!(
      r#"feedrv3_due_feeds_total{{category=\"{}\"}} {}\n"#,
      escape_label(&category),
      count
    ));
  }

  let due_current = metrics
    .due_feeds_current_by_category
    .lock()
    .unwrap_or_else(|e| e.into_inner());

  out.push_str(
    "# HELP feedrv3_due_feeds \\\n     Current due feeds per \\
     category.\n",
  );

  out.push_str(
    "# TYPE feedrv3_due_feeds \\
     gauge\n"
  );

  for (category, count) in
    sorted_map(&due_current)
  {
    out.push_str(&format!(
      r#"feedrv3_due_feeds{{category=\"{}\"}} {}\n"#,
      escape_label(&category),
      count
    ));
  }

  out.push_str(
    "# HELP feedrv3_inflight_actions \
     In-flight feed actions.\n"
  );

  out.push_str(
    "# TYPE feedrv3_inflight_actions \
     gauge\n"
  );

  out.push_str(&format!(
    "feedrv3_inflight_actions {}\n",
    metrics
      .inflight_actions
      .load(Ordering::Relaxed)
  ));

  out.push_str(
    "# HELP feedrv3_feed_actions_total Feed actions by type and outcome.\n",
  );

  out.push_str(
    "# TYPE feedrv3_feed_actions_total counter\n",
  );

  out.push_str(&format!(
    "feedrv3_feed_actions_total{{action=\"head\",outcome=\"ok\"}} {}\n",
    metrics.head_ok.load(Ordering::Relaxed)
  ));

  out.push_str(&format!(
    "feedrv3_feed_actions_total{{action=\"head\",outcome=\"err\"}} {}\n",
    metrics.head_err.load(Ordering::Relaxed)
  ));

  out.push_str(&format!(
    "feedrv3_feed_actions_total{{action=\"get\",outcome=\"ok\"}} {}\n",
    metrics.get_ok.load(Ordering::Relaxed)
  ));

  out.push_str(&format!(
    "feedrv3_feed_actions_total{{action=\"get\",outcome=\"err\"}} {}\n",
    metrics.get_err.load(Ordering::Relaxed)
  ));

  let status_counts = metrics
    .status_counts
    .lock()
    .unwrap_or_else(|e| e.into_inner());

  out.push_str(
    "# HELP feedrv3_http_status_total \
     HTTP status counts by action.\n"
  );

  out.push_str(
    "# TYPE feedrv3_http_status_total \
     counter\n"
  );

  for ((action, status), count) in
    sorted_kv_map(&status_counts)
  {
    out.push_str(&format!(
      "feedrv3_http_status_total{{action=\"{}\",status=\"{}\"}} {}\n",
      escape_label(&action),
      escape_label(&status),
      count
    ));
  }

  let http_latency = metrics
    .http_latency
    .lock()
    .unwrap_or_else(|e| e.into_inner());

  out.push_str(
    "# HELP feedrv3_http_latency_ms \\
     HTTP latency by \\
     action/domain.\n"
  );

  out.push_str(
    "# TYPE feedrv3_http_latency_ms \\
     histogram\n"
  );

  for ((action, domain), hist) in
    sorted_hist_map(&http_latency)
  {
    emit_histogram(
      &mut out,
      "feedrv3_http_latency_ms",
      &LATENCY_BUCKETS_MS,
      &hist,
      &[
        ("action", &action),
        ("domain", &domain)
      ]
    );
  }

  let db_timings = metrics
    .db_timings
    .lock()
    .unwrap_or_else(|e| e.into_inner());

  out.push_str(
    "# HELP feedrv3_db_query_ms \\
     Database query timings.\n"
  );

  out.push_str(
    "# TYPE feedrv3_db_query_ms \\
     histogram\n"
  );

  for (query, hist) in
    sorted_hist_map_single(&db_timings)
  {
    emit_histogram(
      &mut out,
      "feedrv3_db_query_ms",
      &DB_BUCKETS_MS,
      &hist,
      &[("query", &query)]
    );
  }

  out
}

fn escape_label(value: &str) -> String {
  value
    .replace('\\', "\\\\")
    .replace('"', "\\\"")
}

fn sorted_map(
  map: &HashMap<String, u64>
) -> Vec<(String, u64)> {
  let mut items: Vec<(String, u64)> =
    map
      .iter()
      .map(|(k, v)| (k.clone(), *v))
      .collect();

  items.sort_by(|a, b| a.0.cmp(&b.0));

  items
}

fn sorted_kv_map(
  map: &HashMap<String, u64>
) -> Vec<((String, String), u64)> {
  let mut items: Vec<(
    (String, String),
    u64
  )> = map
    .iter()
    .filter_map(|(k, v)| {
      let mut parts = k.splitn(2, '|');

      let action =
        parts.next()?.to_string();

      let status =
        parts.next()?.to_string();

      Some(((action, status), *v))
    })
    .collect();

  items.sort_by(|a, b| a.0.cmp(&b.0));

  items
}

fn sorted_hist_map(
  map: &HashMap<String, Histogram>
) -> Vec<((String, String), Histogram)>
{
  let mut items: Vec<(
    (String, String),
    Histogram
  )> = map
    .iter()
    .filter_map(|(k, v)| {
      let mut parts = k.splitn(2, '|');

      let action =
        parts.next()?.to_string();

      let domain =
        parts.next()?.to_string();

      Some((
        (action, domain),
        v.clone()
      ))
    })
    .collect();

  items.sort_by(|a, b| a.0.cmp(&b.0));

  items
}

fn sorted_hist_map_single(
  map: &HashMap<String, Histogram>
) -> Vec<(String, Histogram)> {
  let mut items: Vec<(
    String,
    Histogram
  )> = map
    .iter()
    .map(|(k, v)| {
      (k.clone(), v.clone())
    })
    .collect();

  items.sort_by(|a, b| a.0.cmp(&b.0));

  items
}

fn emit_histogram(
  out: &mut String,
  name: &str,
  buckets: &[u64],
  hist: &Histogram,
  labels: &[(&str, &str)]
) {
  let mut cumulative = 0u64;

  for (idx, upper) in
    buckets.iter().enumerate()
  {
    cumulative += hist
      .buckets
      .get(idx)
      .copied()
      .unwrap_or(0);

    out.push_str(&format!(
      "{name}_bucket{} {}\n",
      format_labels(
        labels,
        Some(&upper.to_string()),
      ),
      cumulative
    ));
  }

  cumulative += hist
    .buckets
    .get(buckets.len())
    .copied()
    .unwrap_or(0);

  out.push_str(&format!(
    "{name}_bucket{} {}\n",
    format_labels(labels, Some("+Inf")),
    cumulative
  ));

  out.push_str(&format!(
    "{name}_sum{} {}\n",
    format_labels(labels, None),
    hist.sum
  ));

  out.push_str(&format!(
    "{name}_count{} {}\n",
    format_labels(labels, None),
    hist.count
  ));
}

fn format_labels(
  labels: &[(&str, &str)],
  le: Option<&str>
) -> String {
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
