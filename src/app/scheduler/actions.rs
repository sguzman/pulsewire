use std::sync::Arc;

use crate::domain::hashing::sha256_hex;
use crate::domain::link_state::{LinkPhase, LinkState};
use crate::domain::model::AppConfig;
use crate::feed;
use crate::ports::{http::Http, repo::Repo};
use tracing::warn;

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

    state.phase = LinkPhase::NeedsHead;
    tracing::debug!(feed_id = %feed.id, url = %feed.url, "HEAD request start");
    let res = http.head(&feed.url).await;

    let updated = LinkState::apply_head_result(state, res.clone(), now_ms, rand);

    repo.insert_event(
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
    .await?;

    repo.insert_state(&updated, now_ms, &cfg.timezone, record_history)
        .await?;

    if cfg.max_consecutive_errors > 0
        && updated.consecutive_error_count >= cfg.max_consecutive_errors
    {
        repo.mark_feed_error(
            &feed.id,
            res.error,
            res.status.map(|s| s as i64),
            updated.consecutive_error_count as i64,
            now_ms,
            &cfg.timezone,
        )
        .await?;
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

    let body_changed = res.body.as_ref().map(|b| !b.is_empty()).unwrap_or(false);
    let updated = LinkState::apply_get_result(state, res.clone(), now_ms, body_changed, rand);

    repo.insert_event(
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
    .await?;

    if let Some(body) = res.body.as_ref() {
        let hash = sha256_hex(body);
        match feed::parser::parse(body) {
            Ok(parsed) => {
                repo.insert_payload_with_items(
                    &feed.id,
                    now_ms,
                    res.etag.as_deref(),
                    res.last_modified,
                    Some(&hash),
                    &parsed,
                    &cfg.timezone,
                )
                .await?;
            }
            Err(e) => {
                warn!(feed_id = %feed.id, error = %e, "Failed to parse feed");
            }
        }
    }

    repo.insert_state(&updated, now_ms, &cfg.timezone, record_history)
        .await?;
    if cfg.max_consecutive_errors > 0
        && updated.consecutive_error_count >= cfg.max_consecutive_errors
    {
        repo.mark_feed_error(
            &feed.id,
            res.error,
            res.status.map(|s| s as i64),
            updated.consecutive_error_count as i64,
            now_ms,
            &cfg.timezone,
        )
        .await?;
    }
    Ok(())
}
