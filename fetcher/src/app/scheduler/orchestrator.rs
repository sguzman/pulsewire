use std::time::Instant;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::app::context::AppContext;
use crate::ports::{clock::Clock, http::Http, random::RandomSource, repo::Repo};

use super::concurrency::ConcurrencyGuards;
use super::processing::run_tick;

pub struct Scheduler;

impl Scheduler {
    pub async fn run_forever_by_category<R, H, C, G>(
        ctx: AppContext<R, H, C, G>,
        categories: Vec<String>,
    ) -> Result<(), String>
    where
        R: Repo + ?Sized + 'static,
        H: Http + 'static,
        C: Clock + 'static,
        G: RandomSource + 'static,
    {
        if categories.is_empty() {
            return Err("no categories configured for scheduler".to_string());
        }
        let mut handles = FuturesUnordered::new();
        for category in categories {
            let ctx = ctx.clone();
            let name = category.clone();
            handles.push(tokio::spawn(async move {
                Scheduler::run_forever_category(ctx, name).await
            }));
        }

        while let Some(handle) = handles.next().await {
            match handle {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(format!("category task join error: {e}")),
            }
        }
        Ok(())
    }

    pub async fn run_forever_category<R, H, C, G>(
        ctx: AppContext<R, H, C, G>,
        category: String,
    ) -> Result<(), String>
    where
        R: Repo + ?Sized + 'static,
        H: Http + 'static,
        C: Clock + 'static,
        G: RandomSource + 'static,
    {
        let tick_interval = std::time::Duration::from_secs(5);
        let cfg = ctx.cfg.clone();
        let concurrency = ConcurrencyGuards::new(cfg.clone());
        let mut interval = tokio::time::interval(tick_interval);

        loop {
            interval.tick().await;
            let tick_started = Instant::now();
            run_tick(&ctx, &concurrency, tick_started, &category).await?;
        }
    }
}
