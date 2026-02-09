use std::time::Instant;

use super::concurrency::ConcurrencyGuards;
use super::processing::run_tick;
use crate::app::context::AppContext;
use crate::ports::clock::Clock;
use crate::ports::http::Http;
use crate::ports::random::RandomSource;
use crate::ports::repo::Repo;

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

    let tick_interval =
      std::time::Duration::from_secs(5);

    let cfg = ctx.cfg.clone();

    let concurrency =
      ConcurrencyGuards::new(cfg);

    let mut interval =
      tokio::time::interval(
        tick_interval
      );

    loop {
      interval.tick().await;

      for category in &categories {
        let tick_started = Instant::now();

        run_tick(
          &ctx,
          &concurrency,
          tick_started,
          category,
        )
        .await?;
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
    let tick_interval =
      std::time::Duration::from_secs(5);

    let cfg = ctx.cfg.clone();

    let concurrency =
      ConcurrencyGuards::new(
        cfg.clone()
      );

    let mut interval =
      tokio::time::interval(
        tick_interval
      );

    loop {
      interval.tick().await;

      let tick_started = Instant::now();

      run_tick(
        &ctx,
        &concurrency,
        tick_started,
        &category,
      )
      .await?;
    }
  }
}
