//! Row structs and conversions between SQLx records and domain types.
use crate::domain::model::FeedConfig;
use crate::ports::repo::StateRow;

#[derive(Debug, sqlx::FromRow)]
pub struct StateRowRecord {
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

#[derive(Debug, sqlx::FromRow)]
pub struct DueFeedRow {
    pub id: String,
    pub url: String,
    pub domain: String,
    pub category: String,
    pub base_poll_seconds: i64,
}

impl From<StateRowRecord> for StateRow {
    fn from(value: StateRowRecord) -> Self {
        Self {
            feed_id: value.feed_id,
            phase: value.phase,
            last_head_at_ms: value.last_head_at_ms,
            last_head_status: value.last_head_status,
            last_head_error: value.last_head_error,
            last_get_at_ms: value.last_get_at_ms,
            last_get_status: value.last_get_status,
            last_get_error: value.last_get_error,
            etag: value.etag,
            last_modified_ms: value.last_modified_ms,
            backoff_index: value.backoff_index,
            base_poll_seconds: value.base_poll_seconds,
            next_action_at_ms: value.next_action_at_ms,
            jitter_seconds: value.jitter_seconds,
            note: value.note,
            consecutive_error_count: value.consecutive_error_count,
        }
    }
}

impl From<DueFeedRow> for FeedConfig {
    fn from(row: DueFeedRow) -> Self {
        FeedConfig {
            id: row.id,
            url: row.url,
            domain: row.domain,
            category: row.category,
            base_poll_seconds: row.base_poll_seconds.max(0) as u64,
        }
    }
}
