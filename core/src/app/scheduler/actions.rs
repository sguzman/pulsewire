use std::sync::Arc;
use std::time::Instant;

use crate::domain::hashing::sha256_hex;
use crate::domain::link_state::{LinkPhase, LinkState};
use crate::domain::model::AppConfig;
use crate::feed;
use crate::infra::metrics;
use crate::ports::{http::Http, repo::Repo};
use tracing::{error, warn};

use super::concurrency::ConcurrencyGuards;

pub async fn do_head<R, H>(
    cfg: &AppConfig,
    repo: &Arc<R>,
    http: &Arc<H>,
    concurrency: &ConcurrencyGuards,
    feed: &crate::domain::model::FeedConfig,
    mut state: LinkState,
    now_ms: i64,
    rand: f64,
    record_history: bool,
) -> Result<(), String>
where
    R: Repo + ?Sized,
    H: Http,
{
    let _permit = concurrency.permit(&feed.domain).await;
    let _inflight = metrics::record_inflight_start();
    let _inflight = metrics::record_inflight_start();

    state.phase = LinkPhase::NeedsHead;
    tracing::debug!(feed_id = %feed.id, url = %feed.url, "HEAD request start");
    let res = http.head(&feed.url).await;
    metrics::record_http_result(
        "head",
        &feed.domain,
        res.status,
        res.latency_ms,
        res.error.is_none(),
    );

    let updated = LinkState::apply_head_result(state, res.clone(), now_ms, rand);

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
        &cfg.timezone,
    )
    .await;
    metrics::record_db_time("insert_event", started.elapsed().as_millis() as u64);
    event_res?;

    let started = Instant::now();
    let state_res = repo
        .insert_state(&updated, now_ms, &cfg.timezone, record_history)
        .await;
    metrics::record_db_time("insert_state", started.elapsed().as_millis() as u64);
    state_res?;

    if is_immediate_error(cfg, res.status) {
        error!(
            feed_id = %feed.id,
            status = res.status,
            "Feed hit immediate error status"
        );
        let started = Instant::now();
        let err_res = repo
            .mark_feed_error(
            &feed.id,
            res.error,
            res.status.map(|s| s as i64),
            updated.consecutive_error_count as i64,
            now_ms,
            &cfg.timezone,
        )
        .await;
        metrics::record_db_time("mark_feed_error", started.elapsed().as_millis() as u64);
        err_res?;
    } else if cfg.max_consecutive_errors > 0
        && updated.consecutive_error_count >= cfg.max_consecutive_errors
    {
        error!(
            feed_id = %feed.id,
            errors = updated.consecutive_error_count,
            max_errors = cfg.max_consecutive_errors,
            "Feed reached max consecutive errors"
        );
        let started = Instant::now();
        let err_res = repo
            .mark_feed_error(
            &feed.id,
            res.error,
            res.status.map(|s| s as i64),
            updated.consecutive_error_count as i64,
            now_ms,
            &cfg.timezone,
        )
        .await;
        metrics::record_db_time("mark_feed_error", started.elapsed().as_millis() as u64);
        err_res?;
    }

    Ok(())
}

pub async fn do_get<R, H>(
    cfg: &AppConfig,
    repo: &Arc<R>,
    http: &Arc<H>,
    concurrency: &ConcurrencyGuards,
    feed: &crate::domain::model::FeedConfig,
    mut state: LinkState,
    now_ms: i64,
    rand: f64,
    record_history: bool,
) -> Result<(), String>
where
    R: Repo + ?Sized,
    H: Http,
{
    let _permit = concurrency.permit(&feed.domain).await;

    state.phase = LinkPhase::NeedsGet;
    tracing::debug!(feed_id = %feed.id, url = %feed.url, "GET request start");
    let res = http.get(&feed.url).await;
    metrics::record_http_result(
        "get",
        &feed.domain,
        res.status,
        res.latency_ms,
        res.error.is_none(),
    );

    let body_changed = res.body.as_ref().map(|b| !b.is_empty()).unwrap_or(false);
    let updated = LinkState::apply_get_result(state, res.clone(), now_ms, body_changed, rand);

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
        &cfg.timezone,
    )
    .await;
    metrics::record_db_time("insert_event", started.elapsed().as_millis() as u64);
    event_res?;

    if let Some(body) = res.body.as_ref() {
        let hash = sha256_hex(body);
        match feed::parser::parse(body) {
            Ok(parsed) => {
                let started = Instant::now();
                let payload_res = repo
                    .insert_payload_with_items(
                    &feed.id,
                    now_ms,
                    res.etag.as_deref(),
                    res.last_modified,
                    Some(&hash),
                    &parsed,
                    &cfg.timezone,
                )
                .await;
                metrics::record_db_time(
                    "insert_payload_with_items",
                    started.elapsed().as_millis() as u64,
                );
                payload_res?;
            }
            Err(e) => {
                warn!(feed_id = %feed.id, error = %e, "Failed to parse feed");
            }
        }
    }

    let started = Instant::now();
    let state_res = repo
        .insert_state(&updated, now_ms, &cfg.timezone, record_history)
        .await;
    metrics::record_db_time("insert_state", started.elapsed().as_millis() as u64);
    state_res?;
    if is_immediate_error(cfg, res.status) {
        error!(
            feed_id = %feed.id,
            status = res.status,
            "Feed hit immediate error status"
        );
        let started = Instant::now();
        let err_res = repo
            .mark_feed_error(
            &feed.id,
            res.error,
            res.status.map(|s| s as i64),
            updated.consecutive_error_count as i64,
            now_ms,
            &cfg.timezone,
        )
        .await;
        metrics::record_db_time("mark_feed_error", started.elapsed().as_millis() as u64);
        err_res?;
    } else if cfg.max_consecutive_errors > 0
        && updated.consecutive_error_count >= cfg.max_consecutive_errors
    {
        error!(
            feed_id = %feed.id,
            errors = updated.consecutive_error_count,
            max_errors = cfg.max_consecutive_errors,
            "Feed reached max consecutive errors"
        );
        let started = Instant::now();
        let err_res = repo
            .mark_feed_error(
            &feed.id,
            res.error,
            res.status.map(|s| s as i64),
            updated.consecutive_error_count as i64,
            now_ms,
            &cfg.timezone,
        )
        .await;
        metrics::record_db_time("mark_feed_error", started.elapsed().as_millis() as u64);
        err_res?;
    }
    Ok(())
}

fn is_immediate_error(cfg: &AppConfig, status: Option<u16>) -> bool {
    let Some(code) = status else {
        return false;
    };
    cfg.immediate_error_statuses.iter().any(|s| *s == code)
}
