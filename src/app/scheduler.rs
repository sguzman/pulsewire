use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{stream, StreamExt};
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use tracing::{debug, info, warn};

use crate::app::context::AppContext;
use crate::domain::hashing::sha256_hex;
use crate::domain::link_state::{LinkPhase, LinkState, NextAction};
use crate::domain::model::{AppConfig, ErrorKind, FeedConfig};
use crate::feed;
use crate::infra::time::format_epoch_ms;
use crate::ports::{
    clock::Clock,
    http::Http,
    random::RandomSource,
    repo::{Repo, StateRow},
};

pub struct Scheduler;

impl Scheduler {
    /// Runs the scheduler indefinitely: every tick it fetches due feeds from the repo,
    /// processes them with bounded parallelism, and applies the link-state machine
    /// to decide HEAD vs GET vs sleep. Errors are logged and the loop continues.
    pub async fn run_forever<R, H, C, G>(ctx: AppContext<R, H, C, G>) -> Result<(), String>
    where
        R: Repo + 'static,
        H: Http + 'static,
        C: Clock + 'static,
        G: RandomSource + 'static,
    {
        let tick_interval = Duration::from_secs(5);
        let due_batch_size: i64 = 1000;
        let default_parallelism: usize = 64;

        let cfg = ctx.cfg.clone();
        let concurrency = ConcurrencyGuards::new(cfg.clone());

        let mut interval = tokio::time::interval(tick_interval);
        loop {
            interval.tick().await;
            let tick_started = Instant::now();

            let now_ms = ctx.clock.now_epoch_ms().await;
            let due_started = Instant::now();
            let due = ctx.repo.due_feeds(now_ms, due_batch_size).await?;
            let due_elapsed = due_started.elapsed();

            info!(
              tick_time = %format_epoch_ms(now_ms, &cfg.timezone),
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

            // Process feeds concurrently, but keep bounded parallelism.
            stream::iter(due)
                .map(|feed| {
                    let cfg = cfg.clone();
                    let repo = repo.clone();
                    let http = http.clone();
                    let clock = clock.clone();
                    let rng = rng.clone();
                    let concurrency = concurrency.clone();

                    async move {
                        if let Err(e) =
                            process_feed(cfg, repo, http, clock, rng, concurrency, feed).await
                        {
                            warn!(error = %e, "process_feed failed");
                        }
                    }
                })
                .buffer_unordered(parallelism)
                .collect::<Vec<_>>()
                .await;

            info!(
              tick_time = %format_epoch_ms(now_ms, &cfg.timezone),
              total_ms = tick_started.elapsed().as_millis(),
              "Scheduler tick complete"
            );
        }
    }
}

struct PermitPair {
    _g: Option<OwnedSemaphorePermit>,
    _d: OwnedSemaphorePermit,
}

impl PermitPair {
    async fn acquire(global: Option<Arc<Semaphore>>, domain: Arc<Semaphore>) -> Self {
        // Always acquire in the same order to avoid deadlocks: global then domain.
        let g = match global {
            Some(s) => Some(s.acquire_owned().await.expect("global semaphore closed")),
            None => None,
        };
        let d = domain
            .acquire_owned()
            .await
            .expect("domain semaphore closed");
        Self { _g: g, _d: d }
    }
}

async fn process_feed<R, H, C, G>(
    cfg: Arc<AppConfig>,
    repo: Arc<R>,
    http: Arc<H>,
    clock: Arc<C>,
    rng: Arc<G>,
    concurrency: ConcurrencyGuards,
    feed: FeedConfig,
) -> Result<(), String>
where
    R: Repo,
    H: Http,
    C: Clock,
    G: RandomSource,
{
    let now_ms = clock.now_epoch_ms().await;
    let rand = rng.next_f64().await;

    let stored = repo.latest_state(&feed.id).await?;
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

    debug!(
      feed_id = %feed.id,
      action = %describe_action(&action, &cfg),
      now = %format_epoch_ms(now_ms, &cfg.timezone),
      "Decided next action"
    );

    match action {
        NextAction::SleepUntil { .. } => Ok(()),
        NextAction::DoHead { state } => {
            let record_history = should_record_history(&cfg, rng.as_ref()).await;
            do_head(
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
            .await
        }
        NextAction::DoGet { state } => {
            let record_history = should_record_history(&cfg, rng.as_ref()).await;
            do_get(
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
            .await
        }
    }
}

async fn do_head<R, H>(
    cfg: &AppConfig,
    repo: &Arc<R>,
    http: &Arc<H>,
    concurrency: &ConcurrencyGuards,
    feed: &FeedConfig,
    mut state: LinkState,
    now_ms: i64,
    rand: f64,
    record_history: bool,
) -> Result<(), String>
where
    R: Repo,
    H: Http,
{
    let _permit = concurrency.permit(&feed.domain).await;

    state.phase = LinkPhase::NeedsHead;
    debug!(feed_id = %feed.id, url = %feed.url, "HEAD request start");
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

    Ok(())
}

async fn do_get<R, H>(
    cfg: &AppConfig,
    repo: &Arc<R>,
    http: &Arc<H>,
    concurrency: &ConcurrencyGuards,
    feed: &FeedConfig,
    mut state: LinkState,
    now_ms: i64,
    rand: f64,
    record_history: bool,
) -> Result<(), String>
where
    R: Repo,
    H: Http,
{
    let _permit = concurrency.permit(&feed.domain).await;

    state.phase = LinkPhase::NeedsGet;
    debug!(feed_id = %feed.id, url = %feed.url, "GET request start");
    let res = http.get(&feed.url).await;

    let body_changed = res.body.as_ref().map(|b| !b.is_empty()).unwrap_or(false); // same heuristic as Scala :contentReference[oaicite:5]{index=5}
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
                // Keep going; record state anyway.
                warn!(feed_id = %feed.id, error = %e, "Failed to parse feed");
            }
        }
    }

    repo.insert_state(&updated, now_ms, &cfg.timezone, record_history)
        .await?;
    Ok(())
}

fn describe_action(action: &NextAction, cfg: &AppConfig) -> String {
    match action {
        NextAction::SleepUntil { at_ms } => {
            format!("sleep-until {}", format_epoch_ms(*at_ms, &cfg.timezone))
        }
        NextAction::DoHead { .. } => "do-head".to_string(),
        NextAction::DoGet { .. } => "do-get".to_string(),
    }
}

fn to_link_state(row: &StateRow, cfg: &AppConfig) -> Option<LinkState> {
    let phase = parse_phase(&row.phase)?;
    Some(LinkState {
        feed_id: row.feed_id.clone(),
        phase,
        last_head_at_ms: row.last_head_at_ms,
        last_head_status: row.last_head_status.map(|x| x as u16),
        last_head_error: row.last_head_error.as_deref().and_then(parse_error),
        last_get_at_ms: row.last_get_at_ms,
        last_get_status: row.last_get_status.map(|x| x as u16),
        last_get_error: row.last_get_error.as_deref().and_then(parse_error),
        etag: row.etag.clone(),
        last_modified_ms: row.last_modified_ms,
        backoff_index: row.backoff_index.max(0) as u32,
        base_poll_seconds: row.base_poll_seconds.max(0) as u64,
        max_poll_seconds: cfg.max_poll_seconds,
        jitter_fraction: cfg.jitter_fraction,
        next_action_at_ms: row.next_action_at_ms,
        jitter_seconds: row.jitter_seconds,
        note: row.note.clone(),
    })
}

fn parse_error(s: &str) -> Option<ErrorKind> {
    match s {
        "Timeout" => Some(ErrorKind::Timeout),
        "DnsFailure" => Some(ErrorKind::DnsFailure),
        "ConnectionFailure" => Some(ErrorKind::ConnectionFailure),
        "Http4xx" => Some(ErrorKind::Http4xx),
        "Http5xx" => Some(ErrorKind::Http5xx),
        "ParseError" => Some(ErrorKind::ParseError),
        "Unexpected" => Some(ErrorKind::Unexpected),
        _ => None,
    }
}

#[derive(Clone)]
struct ConcurrencyGuards {
    global: Option<Arc<Semaphore>>,
    domains: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    cfg: Arc<AppConfig>,
}

impl ConcurrencyGuards {
    fn new(cfg: Arc<AppConfig>) -> Self {
        let mut per: HashMap<String, Arc<Semaphore>> = HashMap::new();
        for (domain, dcfg) in &cfg.domains {
            per.insert(
                domain.clone(),
                Arc::new(Semaphore::new(dcfg.max_concurrent_requests)),
            );
        }
        let global = cfg
            .global_max_concurrent_requests
            .map(|n| Arc::new(Semaphore::new(n)));
        Self {
            global,
            domains: Arc::new(RwLock::new(per)),
            cfg,
        }
    }

    async fn permit(&self, domain: &str) -> PermitPair {
        let maybe = { self.domains.read().await.get(domain).cloned() };
        let sem = if let Some(s) = maybe {
            s
        } else {
            let limit = self
                .cfg
                .domains
                .get(domain)
                .map(|d| d.max_concurrent_requests)
                .unwrap_or(1);
            let mut guard = self.domains.write().await;
            guard
                .entry(domain.to_string())
                .or_insert_with(|| Arc::new(Semaphore::new(limit)))
                .clone()
        };
        PermitPair::acquire(self.global.clone(), sem).await
    }
}

async fn should_record_history<G: RandomSource>(cfg: &AppConfig, rng: &G) -> bool {
    if cfg.state_history_sample_rate >= 1.0 {
        return true;
    }
    if cfg.state_history_sample_rate <= 0.0 {
        return false;
    }
    rng.next_f64().await < cfg.state_history_sample_rate
}

fn parse_phase(s: &str) -> Option<LinkPhase> {
    match s {
        "NeedsInitialGet" => Some(LinkPhase::NeedsInitialGet),
        "NeedsHead" => Some(LinkPhase::NeedsHead),
        "NeedsGet" => Some(LinkPhase::NeedsGet),
        "Sleeping" => Some(LinkPhase::Sleeping),
        "ErrorBackoff" => Some(LinkPhase::ErrorBackoff),
        _ => None,
    }
}
// Scheduler tick loop: finds due feeds, throttles per-domain/global concurrency,
// and executes HEAD/GET cycles while persisting state, events, and payloads.
