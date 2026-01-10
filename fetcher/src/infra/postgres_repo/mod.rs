//! Postgres-backed repository implementing persistence for feeds, state, events, and payloads.
mod connection;
mod error_feeds;
mod events;
mod feeds;
mod migrations;
mod models;
mod payloads;
mod state;
mod util;

use chrono_tz::Tz;
use sqlx::PgPool;

use crate::domain::link_state::LinkState;
use crate::domain::model::{ErrorKind, FeedConfig, PostgresConfig};
use crate::feed::parser::ParsedFeed;
use crate::ports::repo::{Repo, StateRow};

pub struct PostgresRepo {
    pool: PgPool,
    timezone: Tz,
}

impl PostgresRepo {
    pub async fn new(cfg: &PostgresConfig, timezone: &Tz) -> Result<Self, String> {
        let pool = connection::create_pool(cfg, timezone).await?;
        Ok(Self {
            pool,
            timezone: timezone.clone(),
        })
    }
}

pub async fn wipe_database(cfg: &PostgresConfig, timezone: &Tz) -> Result<(), String> {
    connection::wipe_database(cfg, timezone).await
}

#[async_trait::async_trait]
impl Repo for PostgresRepo {
    async fn migrate(&self, _zone: &Tz, default_poll_seconds: u64) -> Result<(), String> {
        migrations::migrate(&self.pool, default_poll_seconds).await
    }

    async fn upsert_feeds_bulk(
        &self,
        feeds: Vec<FeedConfig>,
        chunk_size: usize,
        zone: &Tz,
    ) -> Result<(), String> {
        feeds::upsert_feeds_bulk(&self.pool, feeds, chunk_size, zone).await
    }

    async fn upsert_categories(&self, categories: Vec<String>, zone: &Tz) -> Result<(), String> {
        feeds::upsert_categories(&self.pool, &categories, zone).await
    }

    async fn latest_state(&self, feed_id: &str) -> Result<Option<StateRow>, String> {
        state::latest_state(&self.pool, feed_id).await
    }

    async fn due_feeds_for_category(
        &self,
        category: &str,
        now_ms: i64,
        limit: i64,
    ) -> Result<Vec<FeedConfig>, String> {
        feeds::due_feeds(&self.pool, category, now_ms, limit, &self.timezone).await
    }

    async fn insert_state(
        &self,
        state: &LinkState,
        recorded_at_ms: i64,
        zone: &Tz,
        record_history: bool,
    ) -> Result<(), String> {
        state::insert_state(&self.pool, state, recorded_at_ms, zone, record_history).await
    }

    async fn insert_event(
        &self,
        feed_id: &str,
        method: &str,
        status: Option<i64>,
        error_kind: Option<ErrorKind>,
        latency_ms: Option<i64>,
        backoff_index: i64,
        scheduled_next_action_at_ms: i64,
        debug: Option<&str>,
        zone: &Tz,
    ) -> Result<(), String> {
        events::insert_event(
            &self.pool,
            feed_id,
            method,
            status,
            error_kind,
            latency_ms,
            backoff_index,
            scheduled_next_action_at_ms,
            debug,
            zone,
        )
        .await
    }

    async fn insert_payload_with_items(
        &self,
        feed_id: &str,
        fetched_at_ms: i64,
        etag: Option<&str>,
        last_modified_ms: Option<i64>,
        content_hash: Option<&str>,
        parsed: &ParsedFeed,
        zone: &Tz,
    ) -> Result<(), String> {
        payloads::insert_payload_with_items(
            &self.pool,
            feed_id,
            fetched_at_ms,
            etag,
            last_modified_ms,
            content_hash,
            parsed,
            zone,
        )
        .await
    }

    async fn mark_feed_error(
        &self,
        feed_id: &str,
        error_kind: Option<ErrorKind>,
        status: Option<i64>,
        error_count: i64,
        observed_at_ms: i64,
        zone: &Tz,
    ) -> Result<(), String> {
        error_feeds::mark_feed_error(
            &self.pool,
            feed_id,
            error_kind,
            status,
            error_count,
            observed_at_ms,
            zone,
        )
        .await
    }
}
