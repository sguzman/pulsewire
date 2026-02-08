use std::sync::Arc;
use std::time::Instant;

use tracing::{
  error,
  warn
};

use super::concurrency::ConcurrencyGuards;
use crate::domain::hashing::sha256_hex;
use crate::domain::link_state::{
  LinkPhase,
  LinkState
};
use crate::domain::model::{
  AppConfig,
  FeedConfig,
  WatchConfig,
  WatchDetector,
  WatchEmitMode
};
use crate::feed;
use crate::feed::parser::{
  FeedItem,
  FeedMetadata,
  ParsedFeed
};
use crate::infra::metrics;
use crate::ports::http::Http;
use crate::ports::repo::Repo;

#[allow(clippy::too_many_arguments)]
pub async fn do_head<R, H>(
  cfg: &AppConfig,
  repo: &Arc<R>,
  http: &Arc<H>,
  concurrency: &ConcurrencyGuards,
  feed: &FeedConfig,
  mut state: LinkState,
  now_ms: i64,
  rand: f64,
  record_history: bool,
  cookie_header: Option<&str>,
  extra_headers: Option<&std::collections::HashMap<String, String>>
) -> Result<(), String>
where
  R: Repo + ?Sized,
  H: Http
{
  let _permit = concurrency
    .permit(&feed.domain)
    .await;

  let _inflight =
    metrics::record_inflight_start();

  state.phase = LinkPhase::NeedsHead;

  tracing::debug!(feed_id = %feed.id, url = %feed.url, "HEAD request start");

  let res = http
    .head(&feed.url, cookie_header, extra_headers)
    .await;

  metrics::record_http_result(
    "head",
    &feed.domain,
    res.status,
    res.latency_ms,
    res.error.is_none()
  );

  let updated =
    LinkState::apply_head_result(
      state,
      res.clone(),
      now_ms,
      rand
    );

  let started = Instant::now();

  let event_res = repo
    .insert_event(
      &feed.id,
      "HEAD",
      res.status.map(|s| s as i64),
      res.error,
      Some(res.latency_ms as i64),
      updated.backoff_index as i64,
      updated.next_action_at_ms,
      updated.note.as_deref(),
      &cfg.timezone
    )
    .await;

  metrics::record_db_time(
    "insert_event",
    started.elapsed().as_millis()
      as u64
  );

  event_res?;

  persist_response_cookies(
    cfg,
    repo,
    &feed.id,
    cookie_header,
    &res.set_cookie_headers,
    now_ms
  )
  .await?;

  let started = Instant::now();

  let state_res = repo
    .insert_state(
      &updated,
      now_ms,
      &cfg.timezone,
      record_history
    )
    .await;

  metrics::record_db_time(
    "insert_state",
    started.elapsed().as_millis()
      as u64
  );

  state_res?;

  maybe_mark_feed_error(
    cfg,
    repo,
    feed,
    res.error,
    res.status,
    updated.consecutive_error_count,
    now_ms
  )
  .await
}

#[allow(clippy::too_many_arguments)]
pub async fn do_get<R, H>(
  cfg: &AppConfig,
  repo: &Arc<R>,
  http: &Arc<H>,
  concurrency: &ConcurrencyGuards,
  feed: &FeedConfig,
  watch: Option<&WatchConfig>,
  cookie_header: Option<&str>,
  extra_headers: Option<&std::collections::HashMap<String, String>>,
  mut state: LinkState,
  now_ms: i64,
  rand: f64,
  record_history: bool
) -> Result<(), String>
where
  R: Repo + ?Sized,
  H: Http
{
  let _permit = concurrency
    .permit(&feed.domain)
    .await;

  let _inflight =
    metrics::record_inflight_start();

  state.phase = LinkPhase::NeedsGet;

  tracing::debug!(feed_id = %feed.id, url = %feed.url, watch = watch.is_some(), "GET request start");

  let res = http
    .get(&feed.url, cookie_header, extra_headers)
    .await;

  metrics::record_http_result(
    "get",
    &feed.domain,
    res.status,
    res.latency_ms,
    res.error.is_none()
  );

  let body_hash = res
    .body
    .as_ref()
    .map(|body| sha256_hex(body));

  let body_changed =
    compute_body_changed(
      &state,
      &res,
      body_hash.as_deref(),
      watch
    );

  let updated =
    LinkState::apply_get_result(
      state,
      res.clone(),
      now_ms,
      body_changed,
      rand
    );

  let started = Instant::now();

  let event_res = repo
    .insert_event(
      &feed.id,
      "GET",
      res.status.map(|s| s as i64),
      res.error,
      Some(res.latency_ms as i64),
      updated.backoff_index as i64,
      updated.next_action_at_ms,
      updated.note.as_deref(),
      &cfg.timezone
    )
    .await;

  metrics::record_db_time(
    "insert_event",
    started.elapsed().as_millis()
      as u64
  );

  event_res?;

  persist_response_cookies(
    cfg,
    repo,
    &feed.id,
    cookie_header,
    &res.set_cookie_headers,
    now_ms
  )
  .await?;

  if let Some(body) = res.body.as_ref()
  {
    persist_payload(
      cfg,
      repo,
      feed,
      watch,
      now_ms,
      &res,
      body,
      body_hash.as_deref(),
      body_changed
    )
    .await?;
  }

  let started = Instant::now();

  let state_res = repo
    .insert_state(
      &updated,
      now_ms,
      &cfg.timezone,
      record_history
    )
    .await;

  metrics::record_db_time(
    "insert_state",
    started.elapsed().as_millis()
      as u64
  );

  state_res?;

  maybe_mark_feed_error(
    cfg,
    repo,
    feed,
    res.error,
    res.status,
    updated.consecutive_error_count,
    now_ms
  )
  .await
}

#[allow(clippy::too_many_arguments)]
async fn persist_payload<R>(
  cfg: &AppConfig,
  repo: &Arc<R>,
  feed: &FeedConfig,
  watch: Option<&WatchConfig>,
  now_ms: i64,
  res: &crate::domain::model::GetResult,
  body: &[u8],
  body_hash: Option<&str>,
  body_changed: bool
) -> Result<(), String>
where
  R: Repo + ?Sized
{
  let parsed = match feed::parser::parse(
    body
  ) {
    | Ok(parsed) => Some(parsed),
    | Err(parse_err) => {
      if let Some(watch_cfg) = watch {
        if body_changed {
          Some(build_synthetic_watch_payload(
            feed,
            watch_cfg,
            now_ms,
            body_hash,
          ))
        } else {
          tracing::debug!(feed_id = %feed.id, error = %parse_err, "Watch parse failed but no body change detected; skipping synthetic emit");
          None
        }
      } else {
        warn!(feed_id = %feed.id, error = %parse_err, "Failed to parse feed");
        None
      }
    }
  };

  if let Some(parsed) = parsed {
    let started = Instant::now();

    let payload_res = repo
      .insert_payload_with_items(
        &feed.id,
        now_ms,
        res.etag.as_deref(),
        res.last_modified,
        body_hash,
        &parsed,
        &cfg.timezone
      )
      .await;

    metrics::record_db_time(
      "insert_payload_with_items",
      started.elapsed().as_millis()
        as u64
    );

    payload_res?;
  }

  Ok(())
}


async fn persist_response_cookies<R>(
  cfg: &AppConfig,
  repo: &Arc<R>,
  feed_id: &str,
  existing_cookie_header: Option<&str>,
  set_cookie_headers: &[String],
  now_ms: i64
) -> Result<(), String>
where
  R: Repo + ?Sized
{
  if set_cookie_headers.is_empty() {
    return Ok(());
  }

  let merged =
    merge_cookie_header_with_set_cookie(
      existing_cookie_header,
      set_cookie_headers
    );

  let Some(cookie_header) = merged else {
    return Ok(());
  };

  repo.upsert_cookie_header(
    feed_id,
    &cookie_header,
    now_ms,
    &cfg.timezone
  )
  .await
}

fn merge_cookie_header_with_set_cookie(
  existing_cookie_header: Option<&str>,
  set_cookie_headers: &[String]
) -> Option<String> {
  let mut pairs =
    std::collections::BTreeMap::<
      String,
      String
    >::new();

  if let Some(existing) =
    existing_cookie_header
  {
    for segment in existing.split(';') {
      let trimmed = segment.trim();
      let Some((k, v)) =
        trimmed.split_once('=')
      else {
        continue;
      };

      let key = k.trim();
      let val = v.trim();
      if key.is_empty() || val.is_empty() {
        continue;
      }

      pairs.insert(
        key.to_string(),
        val.to_string()
      );
    }
  }

  for set_cookie in set_cookie_headers {
    let first = set_cookie
      .split(';')
      .next()
      .unwrap_or("")
      .trim();

    let Some((k, v)) =
      first.split_once('=')
    else {
      continue;
    };

    let key = k.trim();
    let val = v.trim();

    if key.is_empty() || val.is_empty() {
      continue;
    }

    pairs.insert(
      key.to_string(),
      val.to_string()
    );
  }

  if pairs.is_empty() {
    None
  } else {
    Some(
      pairs
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("; ")
    )
  }
}

fn build_synthetic_watch_payload(
  feed: &FeedConfig,
  watch: &WatchConfig,
  now_ms: i64,
  body_hash: Option<&str>
) -> ParsedFeed {
  let title = watch
    .emit_title
    .clone()
    .unwrap_or_else(|| {
      format!("{} changed", feed.id)
    });

  let guid = body_hash
    .map(|h| format!("{}:{h}", feed.id))
    .unwrap_or_else(|| {
      format!("{}:{}", feed.id, now_ms)
    });

  let summary = match watch.emit_mode {
    | WatchEmitMode::NewItemsOnly => {
      Some(
        "watch detected new items"
          .to_string()
      )
    }
    | WatchEmitMode::AnyChange => {
      Some(
        "watch detected content change"
          .to_string()
      )
    }
    | WatchEmitMode::Digest => {
      Some(
        "watch emitted digest update"
          .to_string()
      )
    }
  };

  ParsedFeed {
    metadata: FeedMetadata {
      title:         Some(
        title.clone()
      ),
      link:          Some(
        feed.url.clone()
      ),
      description:   Some(
        "synthetic payload from \
         ad-hoc watch"
          .to_string()
      ),
      language:      watch
        .language
        .clone(),
      updated_at_ms: Some(now_ms)
    },
    items:    vec![FeedItem {
      title: Some(title),
      link: Some(feed.url.clone()),
      guid: Some(guid),
      published_at_ms: Some(now_ms),
      category: Some(
        feed.category.clone()
      ),
      description: summary.clone(),
      summary
    }]
  }
}

fn compute_body_changed(
  state: &LinkState,
  res: &crate::domain::model::GetResult,
  body_hash: Option<&str>,
  watch: Option<&WatchConfig>
) -> bool {
  if let Some(watch_cfg) = watch {
    let mut changed = false;

    for detector in &watch_cfg.detectors
    {
      match detector {
        WatchDetector::Etag => {
          if let Some(etag) =
            res.etag.as_ref()
          {
            changed = changed
              || state
                .etag
                .as_ref()
                .map(|v| v != etag)
                .unwrap_or(true);
          }
        }
        WatchDetector::LastModified => {
          if let Some(last_modified) =
            res.last_modified
          {
            changed = changed
              || state
                .last_modified_ms
                .map(|v| {
                  v != last_modified
                })
                .unwrap_or(true);
          }
        }
        // Phase 2: no persisted prior
        // body/element hash yet, so we
        // treat a present hash as a
        // potential change signal.
        WatchDetector::ContentLength
        | WatchDetector::ContentHash
        | WatchDetector::ElementHash => {
          changed = changed
            || body_hash.is_some();
        }
      }
    }

    if !changed
      && watch_cfg.fetch_body_on_change
      && body_hash.is_some()
    {
      changed = true;
    }

    return changed;
  }

  res
    .body
    .as_ref()
    .map(|b| !b.is_empty())
    .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
async fn maybe_mark_feed_error<R>(
  cfg: &AppConfig,
  repo: &Arc<R>,
  feed: &FeedConfig,
  error_kind: Option<
    crate::domain::model::ErrorKind
  >,
  status: Option<u16>,
  consecutive_error_count: u32,
  now_ms: i64
) -> Result<(), String>
where
  R: Repo + ?Sized
{
  if is_immediate_error(cfg, status) {
    error!(
      feed_id = %feed.id,
      status = status,
      "Feed hit immediate error status"
    );

    let started = Instant::now();

    let err_res = repo
      .mark_feed_error(
        &feed.id,
        error_kind,
        status.map(|s| s as i64),
        consecutive_error_count as i64,
        now_ms,
        &cfg.timezone
      )
      .await;

    metrics::record_db_time(
      "mark_feed_error",
      started.elapsed().as_millis()
        as u64
    );

    return err_res;
  }

  if cfg.max_consecutive_errors > 0
    && consecutive_error_count
      >= cfg.max_consecutive_errors
  {
    error!(
      feed_id = %feed.id,
      errors = consecutive_error_count,
      max_errors = cfg.max_consecutive_errors,
      "Feed reached max consecutive errors"
    );

    let started = Instant::now();

    let err_res = repo
      .mark_feed_error(
        &feed.id,
        error_kind,
        status.map(|s| s as i64),
        consecutive_error_count as i64,
        now_ms,
        &cfg.timezone
      )
      .await;

    metrics::record_db_time(
      "mark_feed_error",
      started.elapsed().as_millis()
        as u64
    );

    return err_res;
  }

  Ok(())
}

fn is_immediate_error(
  cfg: &AppConfig,
  status: Option<u16>
) -> bool {
  let Some(code) = status else {
    return false;
  };

  cfg
    .immediate_error_statuses
    .contains(&code)
}
