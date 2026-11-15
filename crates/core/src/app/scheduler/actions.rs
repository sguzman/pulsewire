use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use scraper::{
  ElementRef,
  Html,
  Selector
};
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
use crate::ports::repo::{
  Repo,
  WatchObservation
};

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
  extra_headers: Option<
    &std::collections::HashMap<
      String,
      String
    >
  >
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
    .head(
      &feed.url,
      cookie_header,
      extra_headers
    )
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
  extra_headers: Option<
    &std::collections::HashMap<
      String,
      String
    >
  >,
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

  let cap = watch
    .and_then(|w| w.max_body_bytes)
    .and_then(|n| usize::try_from(n).ok())
    .unwrap_or(crate::infra::http_policy::MAX_RESPONSE_BODY_BYTES)
    .min(crate::infra::http_policy::MAX_RESPONSE_BODY_BYTES);

  let mut res = http
    .get_with_limit(
      &feed.url,
      cookie_header,
      extra_headers,
      cap
    )
    .await;
  // Defense-in-depth for injected/test
  // Http adapters.
  if res
    .body
    .as_ref()
    .is_some_and(|b| b.len() > cap)
  {
    res.body = None;
    res.error = Some(crate::domain::model::ErrorKind::Unexpected);
  }

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

  let previous_body_hash = if watch
    .is_some_and(|w| {
      w.fetch_body_on_change
        || w.detectors.contains(
          &WatchDetector::ContentHash
        )
    }) {
    repo
      .latest_payload_content_hash(
        &feed.id
      )
      .await?
  } else {
    None
  };

  let successful_body =
    res.error.is_none()
      && res.status.is_some_and(
        |status| {
          (200..300).contains(&status)
        }
      )
      && res.body.is_some();

  let current_observation =
    if successful_body {
      watch
        .zip(res.body.as_deref())
        .map(|(watch, body)| {
          watch_observation(watch, body)
        })
        .transpose()?
    } else {
      None
    };

  let previous_observation =
    if current_observation.is_some() {
      repo
        .latest_watch_observation(
          &feed.id
        )
        .await?
    } else {
      None
    };

  let body_changed = successful_body
    && (compute_body_changed(
      &state,
      &res,
      body_hash.as_deref(),
      previous_body_hash.as_deref(),
      watch
    ) || watch.is_some_and(|w| {
      observation_changed(
        w,
        current_observation.as_ref(),
        previous_observation.as_ref()
      )
    }));

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
    && successful_body
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

  if let Some(observation) =
    current_observation.as_ref()
  {
    repo
      .upsert_watch_observation(
        &feed.id,
        observation,
        now_ms
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
  let mut emitted_count = None;
  let parsed = match feed::parser::parse(
    body
  ) {
    | Ok(parsed) => Some(parsed),
    | Err(parse_err) => {
      if let Some(watch_cfg) = watch {
        if body_changed {
          let extracted =
            extract_watch_items_from_html(
              feed,
              watch_cfg,
              body,
              now_ms
            );
          let current_count =
            extracted.len() as i64;

          let allow_emit = if let Some(
            minimum
          ) =
            watch_cfg
              .min_item_count_change
              .filter(|value| {
                *value > 0
              }) {
            let previous = repo
                .latest_watch_emitted_count(
                  &feed.id
                )
                .await?;
            item_count_gate(
              Some(minimum),
              current_count,
              previous
            )
          } else {
            true
          };

          if !allow_emit {
            tracing::debug!(
              feed_id = %feed.id,
              current_count,
              "Watch item-count threshold not reached"
            );
            None
          } else {
            let emitted =
            if watch_cfg.emit_mode
              == WatchEmitMode::NewItemsOnly
            {
              let candidates = extracted
                .iter()
                .filter_map(|item| {
                  item.guid.clone()
                })
                .collect::<Vec<_>>();
              let known = repo
                .known_watch_item_guids(
                  &feed.id,
                  &candidates
                )
                .await?
                .into_iter()
                .collect::<HashSet<_>>();
              extracted
                .into_iter()
                .filter(|item| {
                  item.guid
                    .as_ref()
                    .is_some_and(|guid| {
                      !known.contains(guid)
                    })
                })
                .collect()
            } else {
              extracted
            };

            let payload =
            build_synthetic_watch_payload(
              feed, watch_cfg, now_ms,
              body_hash, emitted
            );
            if payload.is_some()
              && watch_cfg
                .min_item_count_change
                .unwrap_or(0)
                > 0
            {
              emitted_count =
                Some(current_count);
            }
            payload
          }
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
        emitted_count,
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

  let Some(cookie_header) = merged
  else {
    return Ok(());
  };

  repo
    .upsert_cookie_header(
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
      if key.is_empty()
        || val.is_empty()
      {
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

    if key.is_empty() || val.is_empty()
    {
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
        .map(|(k, v)| {
          format!("{}={}", k, v)
        })
        .collect::<Vec<_>>()
        .join("; ")
    )
  }
}

fn extract_watch_items_from_html(
  feed: &FeedConfig,
  watch: &WatchConfig,
  body: &[u8],
  now_ms: i64
) -> Vec<FeedItem> {
  let Some(item_selector_raw) =
    watch.item_selector.as_deref()
  else {
    return Vec::new();
  };

  let Some(item_selector) =
    parse_selector(item_selector_raw)
  else {
    return Vec::new();
  };

  let title_selector = watch
    .title_selector
    .as_deref()
    .and_then(parse_selector);

  let link_selector = watch
    .link_selector
    .as_deref()
    .and_then(parse_selector);

  let summary_selector = watch
    .summary_selector
    .as_deref()
    .and_then(parse_selector);

  let published_selector = watch
    .published_selector
    .as_deref()
    .and_then(parse_selector);

  let html =
    String::from_utf8_lossy(body);
  let document =
    Html::parse_document(&html);

  let mut seen = HashSet::new();
  let mut items = Vec::new();

  // Bound work and database lookup
  // input, even for large or malformed
  // HTML.
  let max_items = watch
    .max_items_per_fetch
    .unwrap_or(512)
    .min(512)
    as usize;

  for node in
    document.select(&item_selector)
  {
    if items.len() >= max_items {
      break;
    }
    let link = extract_link(
      &node,
      link_selector.as_ref(),
      &feed.url,
      watch.strip_query_params
    );

    let title = extract_text(
      &node,
      title_selector.as_ref()
    )
    .or_else(|| {
      let own = text_of(&node);
      if own.is_empty() {
        None
      } else {
        Some(own)
      }
    });

    let summary = extract_text(
      &node,
      summary_selector.as_ref()
    );

    let published_at_ms =
      extract_published_at_ms(
        &node,
        published_selector.as_ref(),
        watch
          .published_format
          .as_deref()
      )
      .or(Some(now_ms));

    let identity = resolve_identity(
      &node,
      watch,
      link.as_deref(),
      title.as_deref()
    );

    let Some(identity) = identity
    else {
      continue;
    };

    if !seen.insert(identity.clone()) {
      continue;
    }

    items.push(FeedItem {
      title,
      link,
      guid: Some(format!(
        "{}:{}",
        feed.id, identity
      )),
      published_at_ms,
      category: Some(
        feed.category.clone()
      ),
      description: summary.clone(),
      summary
    });
  }

  items
}

fn parse_selector(
  raw: &str
) -> Option<Selector> {
  Selector::parse(raw).ok()
}

fn extract_text(
  node: &ElementRef<'_>,
  selector: Option<&Selector>
) -> Option<String> {
  let selector = selector?;

  let element =
    node.select(selector).next()?;
  let text = text_of(&element);

  if text.is_empty() {
    None
  } else {
    Some(text)
  }
}

fn text_of(
  node: &ElementRef<'_>
) -> String {
  node
    .text()
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

fn extract_link(
  node: &ElementRef<'_>,
  selector: Option<&Selector>,
  base_url: &str,
  strip_query_params: bool
) -> Option<String> {
  let href = selector
    .and_then(|selector| {
      node
        .select(selector)
        .next()
        .and_then(|node| {
          node.value().attr("href")
        })
    })
    .or_else(|| {
      node.value().attr("href")
    })?;

  resolve_watch_href(
    href,
    base_url,
    strip_query_params
  )
}

fn resolve_watch_href(
  href: &str,
  base_url: &str,
  strip_query_params: bool
) -> Option<String> {
  // URL joining follows the document
  // origin/path, not raw string
  // concatenation. Reject non-HTTP
  // targets, including javascript:.
  let base =
    reqwest::Url::parse(base_url)
      .ok()?;
  let mut target =
    base.join(href).ok()?;
  if !matches!(
    target.scheme(),
    "http" | "https"
  ) {
    return None;
  }

  if strip_query_params {
    target.set_query(None);
  }
  Some(target.to_string())
}

fn extract_published_at_ms(
  node: &ElementRef<'_>,
  selector: Option<&Selector>,
  published_format: Option<&str>
) -> Option<i64> {
  let selector = selector?;
  let elem =
    node.select(selector).next()?;

  let raw = elem
    .value()
    .attr("datetime")
    .map(str::to_string)
    .or_else(|| {
      let t = text_of(&elem);
      if t.is_empty() {
        None
      } else {
        Some(t)
      }
    })?;

  if let Some(fmt) = published_format
    && let Ok(dt) =
      chrono::DateTime::parse_from_str(
        &raw, fmt
      )
  {
    return Some(dt.timestamp_millis());
  }

  chrono::DateTime::parse_from_rfc3339(
    &raw
  )
  .map(|dt| dt.timestamp_millis())
  .ok()
}

fn resolve_identity(
  node: &ElementRef<'_>,
  watch: &WatchConfig,
  link: Option<&str>,
  title: Option<&str>
) -> Option<String> {
  match watch.item_identity {
    | Some(
      crate::domain::model::WatchItemIdentity::Href,
    ) => {
      link.map(str::to_string)
    }
    | Some(
      crate::domain::model::WatchItemIdentity::Text,
    ) => {
      title
        .map(str::to_string)
        .or_else(|| {
          let t = text_of(node);
          if t.is_empty() {
            None
          } else {
            Some(t)
          }
        })
    }
    | Some(
      crate::domain::model::WatchItemIdentity::Attr,
    ) => {
      let attr = watch
        .item_identity_attr
        .as_deref()?;
      node.value()
        .attr(attr)
        .map(str::to_string)
    }
    | None => {
      link
        .map(str::to_string)
        .or_else(|| {
          title.map(str::to_string)
        })
    }
  }
}

fn item_count_gate(
  minimum: Option<u64>,
  current_count: i64,
  previous_emitted: Option<i64>
) -> bool {
  let Some(threshold) =
    minimum.filter(|value| *value > 0)
  else {
    return true;
  };

  let current =
    current_count.max(0) as u64;
  let previous = previous_emitted
    .unwrap_or(0)
    .max(0) as u64;
  current.abs_diff(previous)
    >= threshold
}

fn build_synthetic_watch_payload(
  feed: &FeedConfig,
  watch: &WatchConfig,
  now_ms: i64,
  body_hash: Option<&str>,
  extracted_items: Vec<FeedItem>
) -> Option<ParsedFeed> {
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

  let items = match watch.emit_mode {
    | WatchEmitMode::NewItemsOnly => {
      if extracted_items.is_empty() {
        return None;
      }
      extracted_items
    }
    | WatchEmitMode::AnyChange
      if !extracted_items
        .is_empty() =>
    {
      extracted_items
    }
    | WatchEmitMode::AnyChange => {
      vec![FeedItem {
        title:           Some(
          title.clone()
        ),
        link:            Some(
          feed.url.clone()
        ),
        guid:            Some(guid),
        published_at_ms: Some(now_ms),
        category:        Some(
          feed.category.clone()
        ),
        description:     Some(
          "watch detected content \
           change"
            .to_string()
        ),
        summary:         Some(
          "watch detected content \
           change"
            .to_string()
        )
      }]
    }
    | WatchEmitMode::Digest => {
      let preview = extracted_items
        .iter()
        .take(8)
        .map(|item| {
          item
            .title
            .as_deref()
            .or(item.link.as_deref())
            .unwrap_or("Untitled")
            .chars()
            .take(100)
            .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" | ");
      let summary =
        if preview.is_empty() {
          "watch content changed with \
           no matching items"
            .to_string()
        } else {
          format!(
            "{} matching items: \
             {preview}",
            extracted_items.len()
          )
        };
      vec![FeedItem {
        title:           Some(
          title.clone()
        ),
        link:            Some(
          feed.url.clone()
        ),
        guid:            Some(guid),
        published_at_ms: Some(now_ms),
        category:        Some(
          feed.category.clone()
        ),
        description:     Some(
          summary.clone()
        ),
        summary:         Some(summary)
      }]
    }
  };

  Some(ParsedFeed {
    metadata: FeedMetadata {
      title:         Some(title),
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
    items
  })
}

fn watch_observation(
  watch: &WatchConfig,
  body: &[u8]
) -> Result<WatchObservation, String> {
  let element_hash =
    if watch.detectors.contains(
      &WatchDetector::ElementHash
    ) {
      Some(element_fingerprint(
        watch, body
      )?)
    } else {
      None
    };

  Ok(WatchObservation {
    content_length: body.len() as i64,
    element_hash
  })
}

fn observation_changed(
  watch: &WatchConfig,
  current: Option<&WatchObservation>,
  previous: Option<&WatchObservation>
) -> bool {
  let Some(current) = current else {
    return false;
  };
  watch.detectors.iter().any(|detector| {
    match detector {
      | WatchDetector::ContentLength => {
        previous
          .map(|old| {
            old.content_length
              != current.content_length
          })
          .unwrap_or(true)
      }
      | WatchDetector::ElementHash => {
        current
          .element_hash
          .as_ref()
          .is_some_and(|hash| {
            previous
              .and_then(|old| {
                old.element_hash.as_ref()
              })
              .map(|old| old != hash)
              .unwrap_or(true)
          })
      }
      | _ => false
    }
  })
}

fn element_fingerprint(
  watch: &WatchConfig,
  body: &[u8]
) -> Result<String, String> {
  let raw =
    String::from_utf8_lossy(body);
  let document =
    Html::parse_document(&raw);

  let include: Vec<&str> = watch
    .include_selectors
    .as_ref()
    .filter(|v| !v.is_empty())
    .map(|v| {
      v.iter()
        .map(String::as_str)
        .collect()
    })
    .unwrap_or_else(|| {
      vec![
        watch
          .item_selector
          .as_deref()
          .unwrap_or("body"),
      ]
    });

  let parse = |raw: &str| {
    Selector::parse(raw).map_err(|_| {
      "invalid HTML-watch CSS selector"
        .to_string()
    })
  };

  let includes = include
    .into_iter()
    .map(parse)
    .collect::<Result<Vec<_>, _>>()?;

  let excludes = watch
    .exclude_selectors
    .iter()
    .flatten()
    .map(|raw| parse(raw))
    .collect::<Result<Vec<_>, _>>()?;

  let mut seen = HashSet::new();
  let mut sections = Vec::new();

  for selector in &includes {
    for element in
      document.select(selector)
    {
      if !seen.insert(element.id()) {
        continue;
      }

      let mut fragments = Vec::new();
      for node in element.descendants()
      {
        let ignored =
          node.ancestors().any(|anc| {
            ElementRef::wrap(anc)
              .is_some_and(|elem| {
                matches!(
                  elem.value().name(),
                  "script"
                    | "style"
                    | "noscript"
                ) || excludes
                  .iter()
                  .any(|s| {
                    s.matches(&elem)
                  })
              })
          });
        if ignored {
          continue;
        }

        match node.value() {
          | scraper::Node::Text(
            text
          ) => {
            fragments
              .push(text.to_string());
          }
          | _ => {
            if let Some(elem) =
              ElementRef::wrap(node)
              && elem.value().name()
                == "a"
              && let Some(href) = elem
                .value()
                .attr("href")
              && let Some(link) =
                resolve_watch_href(
                  href,
                  &watch.url,
                  watch
                    .strip_query_params
                )
            {
              fragments.push(format!(
                "link:{link}"
              ));
            }
          }
        }
      }

      let section =
        if watch.normalize_whitespace {
          fragments
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
        } else {
          fragments.join("")
        };
      sections.push(section);
    }
  }

  Ok(sha256_hex(
    sections.join("\n").as_bytes()
  ))
}

fn compute_body_changed(
  state: &LinkState,
  res: &crate::domain::model::GetResult,
  body_hash: Option<&str>,
  previous_body_hash: Option<&str>,
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
        WatchDetector::ContentHash => {
          if let Some(hash) =
            body_hash
          {
            changed = changed
              || previous_body_hash
                .map(|previous| {
                  previous != hash
                })
                .unwrap_or(true);
          }
        }
        // Compare persisted
        // detector-specific snapshots
        // instead of treating every
        // body as changed.
        WatchDetector::ContentLength
        | WatchDetector::ElementHash => {}
      }
    }

    // When a watch fetches a body, use
    // its persistent fingerprint as a
    // fallback instead of treating
    // every GET as a change. Handles
    // sources that omit HTTP
    // validators.
    if watch_cfg.fetch_body_on_change
      && !watch_cfg
        .detectors
        .iter()
        .any(|d| {
          matches!(
          d,
          WatchDetector::ContentLength
            | WatchDetector::ContentHash
            | WatchDetector::ElementHash
        )
        })
      && let Some(hash) = body_hash
    {
      changed = changed
        || previous_body_hash
          .map(|old| old != hash)
          .unwrap_or(true);
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

#[cfg(test)]
mod watch_link_tests {
  use super::*;

  #[test]
  fn count_threshold_accumulates_until_emission()
   {
    // Watermark is last committed
    // emission, not last fetch. Small
    // increments therefore accumulate.
    assert!(item_count_gate(
      None,
      1,
      Some(100)
    ));
    assert!(item_count_gate(
      Some(0),
      1,
      Some(100)
    ));
    assert!(!item_count_gate(
      Some(3),
      2,
      None
    ));
    assert!(item_count_gate(
      Some(3),
      3,
      None
    ));
    assert!(!item_count_gate(
      Some(3),
      4,
      Some(3)
    ));
    assert!(!item_count_gate(
      Some(3),
      5,
      Some(3)
    ));
    assert!(item_count_gate(
      Some(3),
      6,
      Some(3)
    ));
    assert!(item_count_gate(
      Some(3),
      0,
      Some(3)
    ));
    assert!(!item_count_gate(
      Some(3),
      10,
      Some(10)
    ));
  }

  fn sample_watch_feed() -> FeedConfig {
    FeedConfig {
      id:                "sample-watch"
        .into(),
      url:
        "https://example.test/list"
          .into(),
      domain:            "example.test"
        .into(),
      category:          "news".into(),
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

  #[test]
  fn emission_modes_distinguish_new_items_and_digest()
   {
    let feed = sample_watch_feed();
    let mut watch = observation_watch();
    watch.item_selector =
      Some("a.story".into());
    watch.item_identity = Some(
      crate::domain::model::WatchItemIdentity::Href
    );

    let items =
      extract_watch_items_from_html(
        &feed,
        &watch,
        br#"<a class="story" href="/one">One</a><a class="story" href="/two">Two</a>"#,
        1000
      );
    assert_eq!(items.len(), 2);
    watch.emit_mode =
      WatchEmitMode::NewItemsOnly;
    assert!(
      build_synthetic_watch_payload(
        &feed,
        &watch,
        1000,
        Some("hash-1"),
        Vec::new()
      )
      .is_none(),
      "no new identity means no \
       synthetic item"
    );

    let one_new =
      vec![items[1].clone()];
    let only_new =
      build_synthetic_watch_payload(
        &feed,
        &watch,
        1000,
        Some("hash-1"),
        one_new
      )
      .expect("one unseen item");
    assert_eq!(only_new.items.len(), 1);
    assert_eq!(
      only_new.items[0]
        .title
        .as_deref(),
      Some("Two")
    );

    watch.emit_mode =
      WatchEmitMode::Digest;
    let digest =
      build_synthetic_watch_payload(
        &feed,
        &watch,
        1000,
        Some("hash-2"),
        items.clone()
      )
      .expect("digest emitted");
    assert_eq!(
      digest.items.len(),
      1,
      "digest is a single item per \
       changed page"
    );
    let summary = digest.items[0]
      .summary
      .as_deref()
      .expect("digest summary");
    assert!(
      summary
        .contains("2 matching items")
    );
    assert!(summary.contains("One"));
    assert!(summary.contains("Two"));
    assert_eq!(
      digest.items[0].guid.as_deref(),
      Some("sample-watch:hash-2")
    );

    watch.emit_mode =
      WatchEmitMode::AnyChange;
    let any =
      build_synthetic_watch_payload(
        &feed,
        &watch,
        1000,
        Some("hash-3"),
        items
      )
      .expect("changed list");
    assert_eq!(any.items.len(), 2);
    let fallback =
      build_synthetic_watch_payload(
        &feed,
        &watch,
        1000,
        Some("hash-4"),
        Vec::new()
      )
      .expect(
        "changed page without \
         selectable items"
      );
    assert_eq!(fallback.items.len(), 1);
  }

  #[test]
  fn html_watch_extracts_at_most_512_items()
   {
    let feed = sample_watch_feed();
    let mut watch = observation_watch();
    watch.item_selector =
      Some("a".into());
    watch.item_identity = Some(
      crate::domain::model::WatchItemIdentity::Href
    );
    let html = (0..600)
      .map(|n| {
        format!(
          "<a href='/story/{n}'>Story \
           {n}</a>"
        )
      })
      .collect::<String>();
    let items =
      extract_watch_items_from_html(
        &feed,
        &watch,
        html.as_bytes(),
        1000
      );
    assert_eq!(items.len(), 512);
    watch.max_items_per_fetch = Some(7);
    assert_eq!(
      extract_watch_items_from_html(
        &feed,
        &watch,
        html.as_bytes(),
        1000
      )
      .len(),
      7
    );
  }

  fn observation_watch() -> WatchConfig
  {
    WatchConfig {
      id: "snapshot-watch".into(),
      url: "https://example.test/page".into(),
      domain: "example.test".into(),
      category: "news".into(),
      base_poll_seconds: 60,
      provenance: None,
      tags: None,
      language: None,
      content_type: None,
      cookie_path: None,
      headers_path: None,
      headers: None,
      check_method:
        crate::domain::model::WatchCheckMethod::Get,
      fallback_to_get: false,
      detectors: vec![
        WatchDetector::ContentLength,
        WatchDetector::ElementHash
      ],
      fetch_body_on_change: true,
      max_body_bytes: None,
      max_items_per_fetch: None,
      item_selector: None,
      item_identity: None,
      item_identity_attr: None,
      title_selector: None,
      link_selector: None,
      summary_selector: None,
      published_selector: None,
      published_format: None,
      include_selectors: Some(vec![
        "article".into()
      ]),
      exclude_selectors: Some(vec![
        ".advert".into()
      ]),
      normalize_whitespace: true,
      strip_query_params: true,
      emit_mode: WatchEmitMode::AnyChange,
      emit_title: None,
      min_item_count_change: None
    }
  }

  #[test]
  fn length_detector_uses_prior_observation()
   {
    let mut watch = observation_watch();
    watch.detectors = vec![
      WatchDetector::ContentLength,
    ];
    let current = WatchObservation {
      content_length: 42,
      element_hash:   None
    };
    assert!(observation_changed(
      &watch,
      Some(&current),
      None
    ));
    assert!(!observation_changed(
      &watch,
      Some(&current),
      Some(&current)
    ));
    let old = WatchObservation {
      content_length: 40,
      element_hash:   None
    };
    assert!(observation_changed(
      &watch,
      Some(&current),
      Some(&old)
    ));
  }

  #[test]
  fn element_hash_ignores_css_excluded_noise()
   {
    let mut watch = observation_watch();
    watch.detectors =
      vec![WatchDetector::ElementHash];

    let first = br#"<article>
      <h1>Stable headline</h1>
      <span class="advert">Promo 1</span>
      <a href="/story?id=42">Read</a>
    </article><aside>noisy sidebar</aside>"#;
    let noise = br#"<article>
      <h1> Stable   headline </h1>
      <span class="advert">Promo 2</span>
      <a href="/story?id=99">Read</a>
    </article><aside>different sidebar</aside>"#;
    let changed = br#"<article>
      <h1>New headline</h1>
      <span class="advert">Promo 2</span>
      <a href="/story?id=99">Read</a>
    </article>"#;

    let a = element_fingerprint(
      &watch, first
    )
    .unwrap();
    let b = element_fingerprint(
      &watch, noise
    )
    .unwrap();
    let c = element_fingerprint(
      &watch, changed
    )
    .unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);

    let old = WatchObservation {
      content_length: first.len()
        as i64,
      element_hash:   Some(a)
    };
    let same = WatchObservation {
      content_length: noise.len()
        as i64,
      element_hash:   Some(b)
    };
    assert!(!observation_changed(
      &watch,
      Some(&same),
      Some(&old)
    ));
    assert!(observation_changed(
      &watch,
      Some(&same),
      None
    ));
  }

  #[test]
  fn element_hash_tracks_links_without_query_noise()
   {
    let watch = observation_watch();
    let base =
      br#"<article><a href="/one?x=1">Same</a></article>"#;
    let noisy =
      br#"<article><a href="/one?x=2">Same</a></article>"#;
    let new_link =
      br#"<article><a href="/two?x=2">Same</a></article>"#;
    assert_eq!(
      element_fingerprint(&watch, base)
        .unwrap(),
      element_fingerprint(
        &watch, noisy
      )
      .unwrap()
    );
    assert_ne!(
      element_fingerprint(&watch, base)
        .unwrap(),
      element_fingerprint(
        &watch, new_link
      )
      .unwrap()
    );
  }

  #[test]
  fn content_hash_detector_suppresses_identical_body()
   {
    let mut watch = WatchConfig {
      id: "hash-watch".into(),
      url: "https://news.example.test/page".into(),
      domain: "news.example.test".into(),
      category: "news".into(),
      base_poll_seconds: 60,
      provenance: None,
      tags: None,
      language: None,
      content_type: None,
      cookie_path: None,
      headers_path: None,
      headers: None,
      check_method:
        crate::domain::model::WatchCheckMethod::Get,
      fallback_to_get: false,
      detectors: vec![
        WatchDetector::ContentHash
      ],
      fetch_body_on_change: false,
      max_body_bytes: None,
      max_items_per_fetch: None,
      item_selector: None,
      item_identity: None,
      item_identity_attr: None,
      title_selector: None,
      link_selector: None,
      summary_selector: None,
      published_selector: None,
      published_format: None,
      include_selectors: None,
      exclude_selectors: None,
      normalize_whitespace: false,
      strip_query_params: false,
      emit_mode: WatchEmitMode::AnyChange,
      emit_title: None,
      min_item_count_change: None
    };

    let state = LinkState::initial(
      "hash-watch".into(),
      60,
      600,
      0.0,
      1_000
    );
    let body = b"<html>Stable</html>";
    let hash = sha256_hex(body);
    let response =
      crate::domain::model::GetResult {
        status:             Some(200),
        body:               Some(
          body.to_vec()
        ),
        etag:               None,
        last_modified:      None,
        error:              None,
        latency_ms:         1,
        set_cookie_headers: Vec::new()
      };

    // First successful observation
    // must still create a notification.
    assert!(compute_body_changed(
      &state,
      &response,
      Some(&hash),
      None,
      Some(&watch)
    ));
    assert!(!compute_body_changed(
      &state,
      &response,
      Some(&hash),
      Some(&hash),
      Some(&watch)
    ));
    assert!(compute_body_changed(
      &state,
      &response,
      Some(&hash),
      Some("different-hash"),
      Some(&watch)
    ));

    // fetch_body_on_change is not a
    // detector: it must not override an
    // unchanged content hash.
    watch.fetch_body_on_change = true;
    assert!(!compute_body_changed(
      &state,
      &response,
      Some(&hash),
      Some(&hash),
      Some(&watch)
    ));

    // A second configured detector can
    // independently signal a change.
    watch
      .detectors
      .push(WatchDetector::Etag);
    let mut tagged_state =
      state.clone();
    tagged_state.etag =
      Some("old-etag".into());
    let mut tagged_response =
      response.clone();
    tagged_response.etag =
      Some("new-etag".into());
    assert!(compute_body_changed(
      &tagged_state,
      &tagged_response,
      Some(&hash),
      Some(&hash),
      Some(&watch)
    ));

    // Default watches use ETag and
    // Last-Modified detectors. When a
    // source sends neither, the body
    // still establishes a first change
    // and suppresses identical repeats.
    watch.detectors.clear();
    assert!(compute_body_changed(
      &state,
      &response,
      Some(&hash),
      None,
      Some(&watch)
    ));
    assert!(!compute_body_changed(
      &state,
      &response,
      Some(&hash),
      Some(&hash),
      Some(&watch)
    ));
  }

  #[test]
  fn watch_links_resolve_from_document_url()
   {
    let base =
      "https://news.example.test/archive/index.html";

    assert_eq!(
      resolve_watch_href(
        "/articles/1", base, false
      )
      .as_deref(),
      Some("https://news.example.test/articles/1")
    );
    assert_eq!(
      resolve_watch_href(
        "../articles/2", base, false
      )
      .as_deref(),
      Some("https://news.example.test/articles/2")
    );
    assert_eq!(
      resolve_watch_href(
        "https://other.example.test/story",
        base,
        false
      )
      .as_deref(),
      Some("https://other.example.test/story")
    );
  }

  #[test]
  fn watch_links_strip_query_without_discarding_fragment()
   {
    assert_eq!(
      resolve_watch_href(
        "/article?id=42#section",
        "https://news.example.test/feed",
        true
      )
      .as_deref(),
      Some(
        "https://news.example.test/article#section"
      )
    );
  }

  #[test]
  fn watch_links_reject_non_http_targets()
   {
    for href in [
      "javascript:alert(1)",
      "data:text/html,unsafe",
      "mailto:example@example.test"
    ] {
      assert!(
        resolve_watch_href(
          href,
          "https://news.example.test/feed",
          false
        )
        .is_none()
      );
    }
  }

  #[test]
  fn html_watch_link_selector_resolves_relative_href()
   {
    let html = Html::parse_document(
      r#"<div class="item"><a class="story" href="../post?id=2">Story</a></div>"#
    );
    let item_selector =
      Selector::parse("div.item")
        .expect("valid item selector");
    let link_selector =
      Selector::parse("a.story")
        .expect("valid link selector");
    let item = html
      .select(&item_selector)
      .next()
      .expect("fixture item");

    assert_eq!(
      extract_link(
        &item,
        Some(&link_selector),
        "https://news.example.test/archive/page",
        false
      )
      .as_deref(),
      Some("https://news.example.test/post?id=2")
    );
  }
  #[test]
  fn html_watch_does_not_restore_unsafe_raw_links()
   {
    let feed = FeedConfig {
      id:                "ci-watch".into(),
      url: "https://news.example.test/archive/index.html".into(),
      domain: "news.example.test".into(),
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
      id:                feed.id.clone(),
      url:               feed.url.clone(),
      domain:            feed.domain.clone(),
      category:          feed.category.clone(),
      base_poll_seconds: 60,
      provenance: None,
      tags: None,
      language: None,
      content_type: None,
      cookie_path: None,
      headers_path: None,
      headers: None,
      check_method:
        crate::domain::model::WatchCheckMethod::Get,
      fallback_to_get: false,
      detectors: Vec::new(),
      fetch_body_on_change: false,
      max_body_bytes: None,
      max_items_per_fetch: None,
      item_selector: Some("a".into()),
      item_identity: Some(
        crate::domain::model::WatchItemIdentity::Href
      ),
      item_identity_attr: None,
      title_selector: None,
      link_selector: None,
      summary_selector: None,
      published_selector: None,
      published_format: None,
      include_selectors: None,
      exclude_selectors: None,
      normalize_whitespace: false,
      strip_query_params: false,
      emit_mode: WatchEmitMode::NewItemsOnly,
      emit_title: None,
      min_item_count_change: None
    };

    let unsafe_items =
      extract_watch_items_from_html(
        &feed,
        &watch,
        br#"<a href="javascript:alert(1)">Unsafe</a>"#,
        0
      );
    assert!(unsafe_items.is_empty());

    let safe_items =
      extract_watch_items_from_html(
        &feed,
        &watch,
        br#"<a href="/article">Safe</a>"#,
        0
      );
    assert_eq!(safe_items.len(), 1);
    assert_eq!(
      safe_items[0].link.as_deref(),
      Some("https://news.example.test/article")
    );
  }
}
