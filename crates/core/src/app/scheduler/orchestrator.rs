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

    // Each category owns its tick timer and retry backoff. All loops
    // share the same global and per-domain network concurrency guards.
    let parallelism =
      per_category_parallelism(
        ctx.cfg
          .global_max_concurrent_requests
          .unwrap_or(64),
        categories.len()
      );
    let concurrency =
      ConcurrencyGuards::new(
        ctx.cfg.clone()
      );

    let loops = categories
      .into_iter()
      .map(|category| {
        Self::run_category_loop(
          ctx.clone(),
          category,
          concurrency.clone(),
          parallelism
        )
      });

    futures::future::try_join_all(loops)
      .await?;

    Ok(())
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
    let concurrency =
      ConcurrencyGuards::new(
        ctx.cfg.clone()
      );
    let parallelism = ctx
      .cfg
      .global_max_concurrent_requests
      .unwrap_or(64)
      .max(1);

    Self::run_category_loop(
      ctx,
      category,
      concurrency,
      parallelism
    )
    .await
  }

  async fn run_category_loop<
    R,
    H,
    C,
    G
  >(
    ctx: AppContext<R, H, C, G>,
    category: String,
    concurrency: ConcurrencyGuards,
    parallelism: usize
  ) -> Result<(), String>
  where
    R: Repo + ?Sized + 'static,
    H: Http + 'static,
    C: Clock + 'static,
    G: RandomSource + 'static
  {
    let mut interval =
      tokio::time::interval(
        Duration::from_secs(
          TICK_INTERVAL_SECS
        )
      );

    interval.set_missed_tick_behavior(
      MissedTickBehavior::Skip
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
        parallelism
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
            consecutive_errors
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
  let exp = consecutive_errors
    .min(RETRY_EXP_CAP);

  let mult = 1u64
    .checked_shl(exp)
    .unwrap_or(u64::MAX);

  let secs = RETRY_BASE_SECS
    .saturating_mul(mult)
    .min(RETRY_MAX_SECS);

  Duration::from_secs(secs)
}

fn per_category_parallelism(
  total_parallelism: usize,
  category_count: usize
) -> usize {
  // Per-category windows avoid giving every slot to one busy category.
  // Shared global semaphore still caps aggregate request concurrency.
  total_parallelism
    .max(1)
    .div_ceil(category_count.max(1))
    .max(1)
}

#[cfg(test)]
mod fairness_tests {
  use super::*;

  #[test]
  fn fair_windows_partition_the_global_budget() {
    assert_eq!(per_category_parallelism(64, 4), 16);
    assert_eq!(per_category_parallelism(5, 2), 3);
    assert_eq!(per_category_parallelism(2, 5), 1);
    assert_eq!(per_category_parallelism(0, 3), 1);
    assert_eq!(per_category_parallelism(64, 1), 64);
  }

  #[test]
  fn single_category_and_empty_input_are_bounded() {
    assert_eq!(per_category_parallelism(64, 0), 64);
    assert_eq!(per_category_parallelism(1, 0), 1);
  }
}
