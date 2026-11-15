//! SQLite-backed repository
//! implementing persistence for feeds,
//! state, events, and payloads.

mod connection;
mod cookies;
mod error_feeds;
mod events;
mod feeds;
mod migrations;
mod models;
mod payloads;
mod state;
mod util;

use std::path::Path;

use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::domain::link_state::LinkState;
use crate::domain::model::{
  ErrorKind,
  FeedConfig
};
use crate::feed::parser::ParsedFeed;
use crate::ports::repo::{
  Repo,
  StateRow,
  WatchObservation
};

pub struct SqliteRepo {
  pool: SqlitePool
}

impl SqliteRepo {
  pub async fn new(
    db_path: &Path
  ) -> Result<Self, String> {
    let pool =
      connection::create_pool(db_path)
        .await?;

    Ok(Self {
      pool
    })
  }
}

#[async_trait::async_trait]

impl Repo for SqliteRepo {
  async fn migrate(
    &self,
    zone: &Tz,
    default_poll_seconds: u64
  ) -> Result<(), String> {
    migrations::migrate(
      &self.pool,
      zone,
      default_poll_seconds
    )
    .await
  }

  async fn upsert_feeds_bulk(
    &self,
    feeds: Vec<FeedConfig>,
    chunk_size: usize,
    zone: &Tz
  ) -> Result<(), String> {
    feeds::upsert_feeds_bulk(
      &self.pool, feeds, chunk_size,
      zone
    )
    .await
  }

  async fn upsert_categories(
    &self,
    categories: Vec<String>,
    _zone: &Tz
  ) -> Result<(), String> {
    feeds::upsert_categories(
      &self.pool,
      &categories
    )
    .await
  }

  async fn latest_state(
    &self,
    feed_id: &str
  ) -> Result<Option<StateRow>, String>
  {
    state::latest_state(
      &self.pool, feed_id
    )
    .await
  }

  async fn due_feeds_for_category(
    &self,
    category: &str,
    now_ms: i64,
    limit: i64
  ) -> Result<Vec<FeedConfig>, String>
  {
    feeds::due_feeds(
      &self.pool, category, now_ms,
      limit
    )
    .await
  }

  async fn insert_state(
    &self,
    state: &LinkState,
    recorded_at_ms: i64,
    zone: &Tz,
    record_history: bool
  ) -> Result<(), String> {
    state::insert_state(
      &self.pool,
      state,
      recorded_at_ms,
      zone,
      record_history
    )
    .await
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
    zone: &Tz
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
      zone
    )
    .await
  }

  async fn latest_payload_content_hash(
    &self,
    feed_id: &str
  ) -> Result<Option<String>, String>
  {
    payloads::latest_payload_content_hash(
      &self.pool,
      feed_id
    )
    .await
  }

  async fn known_watch_item_guids(
    &self,
    feed_id: &str,
    candidates: &[String]
  ) -> Result<Vec<String>, String> {
    payloads::known_watch_item_guids(
      &self.pool, feed_id, candidates
    )
    .await
  }

  async fn latest_watch_observation(
    &self,
    feed_id: &str
  ) -> Result<
    Option<WatchObservation>,
    String
  > {
    payloads::latest_watch_observation(
      &self.pool, feed_id
    )
    .await
  }

  async fn upsert_watch_observation(
    &self,
    feed_id: &str,
    observation: &WatchObservation,
    observed_at_ms: i64
  ) -> Result<(), String> {
    payloads::upsert_watch_observation(
      &self.pool,
      feed_id,
      observation,
      observed_at_ms
    )
    .await
  }

  async fn latest_watch_emitted_count(
    &self,
    feed_id: &str
  ) -> Result<Option<i64>, String> {
    payloads::latest_watch_emitted_count(
      &self.pool, feed_id
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
    watch_item_count: Option<i64>,
    zone: &Tz
  ) -> Result<(), String> {
    payloads::insert_payload_with_items(
      &self.pool,
      feed_id,
      fetched_at_ms,
      etag,
      last_modified_ms,
      content_hash,
      parsed,
      watch_item_count,
      zone
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
    zone: &Tz
  ) -> Result<(), String> {
    error_feeds::mark_feed_error(
      &self.pool,
      feed_id,
      error_kind,
      status,
      error_count,
      observed_at_ms,
      zone
    )
    .await
  }

  async fn latest_cookie_header(
    &self,
    feed_id: &str
  ) -> Result<Option<String>, String>
  {
    cookies::latest_cookie_header(
      &self.pool, feed_id
    )
    .await
  }

  async fn upsert_cookie_header(
    &self,
    feed_id: &str,
    cookie_header: &str,
    observed_at_ms: i64,
    zone: &Tz
  ) -> Result<(), String> {
    cookies::upsert_cookie_header(
      &self.pool,
      feed_id,
      cookie_header,
      observed_at_ms,
      zone
    )
    .await
  }
}
