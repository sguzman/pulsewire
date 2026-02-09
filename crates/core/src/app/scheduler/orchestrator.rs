use std::time::{
  Duration,
  Instant
};

use tokio::time::MissedTickBehavior;
use tracing::warn;

use super::concurrency::ConcurrencyGuards;
use super::processing::run_tick;
use crate::app::context::AppContext;
use crate::ports::clock::Clock;
use crate::ports::http::Http;
use crate::ports::random::RandomSource;
use crate::ports::repo::Repo;

const TICK_INTERVAL_SECS: u64 = 5;
const RETRY_BASE_SECS: u64 = 2;
const RETRY_MAX_SECS: u64 = 60;
const RETRY_EXP_CAP: u32 = 5;

pub struct Scheduler;

impl Scheduler {
  pub async fn run_forever_by_category<
    R,
    H,
    C,
    G
  >(
    ctx: AppContext<R, H, C, G>,
    categories: Vec<String>
  ) -> Result<(), String>
  where
    R: Repo + ?Sized + 'static,
    H: Http + 'static,
    C: Clock + 'static,
    G: RandomSource + 'static
  {
    if categories.is_empty() {
      return Err(
        "no categories configured for \
         scheduler"
          .to_string()
      );
    }

    let cfg = ctx.cfg.clone();

    let concurrency =
      ConcurrencyGuards::new(cfg);

    let mut interval =
      tokio::time::interval(
        Duration::from_secs(
          TICK_INTERVAL_SECS
        )
      );

    // If a tick is delayed due to retry
    // backoff, skip missed ticks instead
    // of replaying all of them.
    interval
      .set_missed_tick_behavior(
        MissedTickBehavior::Skip,
      );

    let mut consecutive_errors: u32 = 0;

    loop {
      interval.tick().await;

      let mut tick_failed = false;

      for category in &categories {
        let tick_started = Instant::now();

        match run_tick(
          &ctx,
          &concurrency,
          tick_started,
          category,
        )
        .await
        {
          | Ok(()) => {
            consecutive_errors = 0;
          }
          | Err(error) => {
            consecutive_errors =
              consecutive_errors
                .saturating_add(1);

            let backoff =
              retry_backoff(
                consecutive_errors,
              );

            warn!(
                category = category,
                error = %error,
                consecutive_errors = consecutive_errors,
                backoff_secs = backoff.as_secs(),
                "Scheduler tick failed; continuing with backoff"
            );

            tokio::time::sleep(backoff)
              .await;

            tick_failed = true;
            break;
          }
        }
      }

      if tick_failed {
        continue;
      }
    }
  }

  pub async fn run_forever_category<
    R,
    H,
    C,
    G
  >(
    ctx: AppContext<R, H, C, G>,
    category: String
  ) -> Result<(), String>
  where
    R: Repo + ?Sized + 'static,
    H: Http + 'static,
    C: Clock + 'static,
    G: RandomSource + 'static
  {
    let cfg = ctx.cfg.clone();

    let concurrency =
      ConcurrencyGuards::new(cfg);

    let mut interval =
      tokio::time::interval(
        Duration::from_secs(
          TICK_INTERVAL_SECS
        )
      );

    interval
      .set_missed_tick_behavior(
        MissedTickBehavior::Skip,
      );

    let mut consecutive_errors: u32 = 0;

    loop {
      interval.tick().await;

      let tick_started = Instant::now();

      match run_tick(
        &ctx,
        &concurrency,
        tick_started,
        &category,
      )
      .await
      {
        | Ok(()) => {
          consecutive_errors = 0;
        }
        | Err(error) => {
          consecutive_errors =
            consecutive_errors
              .saturating_add(1);

          let backoff = retry_backoff(
            consecutive_errors,
          );

          warn!(
              category = category,
              error = %error,
              consecutive_errors = consecutive_errors,
              backoff_secs = backoff.as_secs(),
              "Category scheduler tick failed; continuing with backoff"
          );

          tokio::time::sleep(backoff)
            .await;
        }
      }
    }
  }
}

fn retry_backoff(
  consecutive_errors: u32
) -> Duration {
  let exp =
    consecutive_errors.min(
      RETRY_EXP_CAP,
    );

  let mult =
    1u64
      .checked_shl(exp)
      .unwrap_or(u64::MAX);

  let secs = RETRY_BASE_SECS
    .saturating_mul(mult)
    .min(RETRY_MAX_SECS);

  Duration::from_secs(secs)
}
