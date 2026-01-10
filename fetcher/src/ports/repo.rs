//! Repository abstraction for feed definitions, state snapshots, fetch events, and parsed payloads.
use chrono_tz::Tz;

use crate::{
    domain::{
        link_state::LinkState,
        model::{ErrorKind, FeedConfig},
    },
    feed::parser::ParsedFeed,
};

#[derive(Debug, Clone)]
pub struct StateRow {
    pub feed_id: String,
    pub phase: String,
    pub last_head_at_ms: Option<i64>,
    pub last_head_status: Option<i64>,
    pub last_head_error: Option<String>,
    pub last_get_at_ms: Option<i64>,
    pub last_get_status: Option<i64>,
    pub last_get_error: Option<String>,
    pub etag: Option<String>,
    pub last_modified_ms: Option<i64>,
    pub backoff_index: i64,
    pub base_poll_seconds: i64,
    pub next_action_at_ms: i64,
    pub jitter_seconds: i64,
    pub note: Option<String>,
    pub consecutive_error_count: i64,
}

#[async_trait::async_trait]
pub trait Repo: Send + Sync {
    async fn migrate(&self, zone: &Tz, default_poll_seconds: u64) -> Result<(), String>;
    async fn upsert_feeds_bulk(
        &self,
        feeds: Vec<FeedConfig>,
        chunk_size: usize,
        zone: &Tz,
    ) -> Result<(), String>;
    async fn upsert_categories(&self, categories: Vec<String>, zone: &Tz) -> Result<(), String>;

    async fn latest_state(&self, feed_id: &str) -> Result<Option<StateRow>, String>;
    async fn due_feeds_for_category(
        &self,
        category: &str,
        now_ms: i64,
        limit: i64,
    ) -> Result<Vec<FeedConfig>, String>;

    async fn insert_state(
        &self,
        state: &LinkState,
        recorded_at_ms: i64,
        zone: &Tz,
        record_history: bool,
    ) -> Result<(), String>;

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
    ) -> Result<(), String>;

    async fn insert_payload_with_items(
        &self,
        feed_id: &str,
        fetched_at_ms: i64,
        etag: Option<&str>,
        last_modified_ms: Option<i64>,
        content_hash: Option<&str>,
        parsed: &ParsedFeed,
        zone: &Tz,
    ) -> Result<(), String>;

    async fn mark_feed_error(
        &self,
        feed_id: &str,
        error_kind: Option<ErrorKind>,
        status: Option<i64>,
        error_count: i64,
        observed_at_ms: i64,
        zone: &Tz,
    ) -> Result<(), String>;
}
