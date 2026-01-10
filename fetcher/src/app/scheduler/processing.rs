use std::sync::Arc;
use std::time::Instant;

use futures::{stream, StreamExt};
use tracing::{debug, info, warn};

use crate::app::context::AppContext;
use crate::domain::link_state::LinkState;
use crate::domain::model::FeedConfig;
use crate::infra::metrics;
use crate::infra::time::format_epoch_ms;
use crate::ports::{clock::Clock, http::Http, random::RandomSource, repo::Repo};

use super::actions::{do_get, do_head};
use super::concurrency::ConcurrencyGuards;
use super::state::{describe_action, should_record_history, to_link_state};

pub async fn run_tick<R, H, C, G>(
    ctx: &AppContext<R, H, C, G>,
    concurrency: &ConcurrencyGuards,
    tick_started: Instant,
    category: &str,
) -> Result<(), String>
where
    R: Repo + ?Sized + 'static,
    H: Http + 'static,
    C: Clock + 'static,
    G: RandomSource + 'static,
{
    let due_batch_size: i64 = 1000;
    let default_parallelism: usize = 64;

    let cfg = ctx.cfg.clone();

    let now_ms = ctx.clock.now_epoch_ms().await;
    let due_started = Instant::now();
    let due = ctx
        .repo
        .due_feeds_for_category(category, now_ms, due_batch_size)
        .await?;
    let due_elapsed = due_started.elapsed();
    metrics::record_tick(category, due.len() as u64);
    metrics::record_db_time("due_feeds_for_category", due_elapsed.as_millis() as u64);

    info!(
      tick_time = %format_epoch_ms(now_ms, &cfg.timezone),
      category = category,
      due = due.len(),
      due_batch_limit = due_batch_size,
      due_query_ms = due_elapsed.as_millis(),
      "Scheduler tick"
    );

    let parallelism = cfg
        .global_max_concurrent_requests
        .unwrap_or(default_parallelism);
    let repo = ctx.repo.clone();
    let http = ctx.http.clone();
    let clock = ctx.clock.clone();
    let rng = ctx.rng.clone();

    let warn_after = cfg.log_tick_warn_seconds;
    let tick_guard = if warn_after > 0 {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let category = category.to_string();
        let due_count = due.len();
        let timezone = cfg.timezone;
        let tick_time = format_epoch_ms(now_ms, &timezone);
        let started = tick_started;
        tokio::spawn(async move {
            let sleep = tokio::time::sleep(std::time::Duration::from_secs(warn_after));
            tokio::pin!(sleep);
            tokio::select! {
                _ = &mut sleep => {
                    warn!(
                        category = category,
                        due = due_count,
                        tick_time = %tick_time,
                        elapsed_ms = started.elapsed().as_millis(),
                        "Scheduler tick still running"
                    );
                }
                _ = rx => {}
            }
        });
        Some(tx)
    } else {
        None
    };

    stream::iter(due)
        .map(|feed| {
            let cfg = cfg.clone();
            let repo = repo.clone();
            let http = http.clone();
            let clock = clock.clone();
            let rng = rng.clone();
            let concurrency = concurrency.clone();

            async move {
                if let Err(e) = process_feed(cfg, repo, http, clock, rng, concurrency, feed).await {
                    warn!(error = %e, "process_feed failed");
                }
            }
        })
        .buffer_unordered(parallelism)
        .collect::<Vec<_>>()
        .await;

    if let Some(tx) = tick_guard {
        let _ = tx.send(());
    }

    info!(
      tick_time = %format_epoch_ms(now_ms, &cfg.timezone),
      category = category,
      total_ms = tick_started.elapsed().as_millis(),
      "Scheduler tick complete"
    );

    Ok(())
}

async fn process_feed<R, H, C, G>(
    cfg: Arc<crate::domain::model::AppConfig>,
    repo: Arc<R>,
    http: Arc<H>,
    clock: Arc<C>,
    rng: Arc<G>,
    concurrency: ConcurrencyGuards,
    feed: FeedConfig,
) -> Result<(), String>
where
    R: Repo + ?Sized,
    H: Http,
    C: Clock,
    G: RandomSource,
{
    let now_ms = clock.now_epoch_ms().await;
    let rand = rng.next_f64().await;

    let started = Instant::now();
    let stored = repo.latest_state(&feed.id).await?;
    metrics::record_db_time("latest_state", started.elapsed().as_millis() as u64);
    let state = stored
        .and_then(|r| to_link_state(&r, &cfg))
        .unwrap_or_else(|| {
            LinkState::initial(
                feed.id.clone(),
                feed.base_poll_seconds,
                cfg.max_poll_seconds,
                cfg.jitter_fraction,
                now_ms,
            )
        });

    let action = LinkState::decide_next_action(&state, now_ms);
    let log_feed_timing = cfg.log_feed_timing_enabled
        && (cfg.log_feed_timing_domains.is_empty()
            || cfg
                .log_feed_timing_domains
                .iter()
                .any(|d| d == &feed.domain));

    debug!(
      feed_id = %feed.id,
      action = %describe_action(&action, &cfg),
      now = %format_epoch_ms(now_ms, &cfg.timezone),
      "Decided next action"
    );

    match action {
        crate::domain::link_state::NextAction::SleepUntil { .. } => Ok(()),
        crate::domain::link_state::NextAction::DoHead { state } => {
            let record_history = should_record_history(&cfg, rng.as_ref()).await;
            let started = Instant::now();
            let res = do_head(
                &cfg,
                &repo,
                &http,
                &concurrency,
                &feed,
                state,
                now_ms,
                rand,
                record_history,
            )
            .await;
            let elapsed_ms = started.elapsed().as_millis() as u64;
            if log_feed_timing {
                if cfg.log_feed_timing_log_all {
                    info!(
                        feed_id = %feed.id,
                        domain = %feed.domain,
                        action = "HEAD",
                        elapsed_ms,
                        "Feed timing"
                    );
                } else if elapsed_ms >= cfg.log_feed_timing_warn_ms {
                    warn!(
                        feed_id = %feed.id,
                        domain = %feed.domain,
                        action = "HEAD",
                        elapsed_ms,
                        warn_after_ms = cfg.log_feed_timing_warn_ms,
                        "Slow feed action"
                    );
                }
            }
            res
        }
        crate::domain::link_state::NextAction::DoGet { state } => {
            let record_history = should_record_history(&cfg, rng.as_ref()).await;
            let started = Instant::now();
            let res = do_get(
                &cfg,
                &repo,
                &http,
                &concurrency,
                &feed,
                state,
                now_ms,
                rand,
                record_history,
            )
            .await;
            let elapsed_ms = started.elapsed().as_millis() as u64;
            if log_feed_timing {
                if cfg.log_feed_timing_log_all {
                    info!(
                        feed_id = %feed.id,
                        domain = %feed.domain,
                        action = "GET",
                        elapsed_ms,
                        "Feed timing"
                    );
                } else if elapsed_ms >= cfg.log_feed_timing_warn_ms {
                    warn!(
                        feed_id = %feed.id,
                        domain = %feed.domain,
                        action = "GET",
                        elapsed_ms,
                        warn_after_ms = cfg.log_feed_timing_warn_ms,
                        "Slow feed action"
                    );
                }
            }
            res
        }
    }
}
