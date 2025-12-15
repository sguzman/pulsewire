//! Records fetch events (HEAD/GET) with timing/status/error info.
use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::domain::model::ErrorKind;

use super::util::now_epoch_ms;

pub async fn insert_event(
    pool: &SqlitePool,
    feed_id: &str,
    method: &str,
    status: Option<i64>,
    error_kind: Option<ErrorKind>,
    latency_ms: Option<i64>,
    backoff_index: i64,
    scheduled_next_action_at_ms: i64,
    debug: Option<&str>,
    _zone: &Tz,
) -> Result<(), String> {
    let now_ms = now_epoch_ms();
    sqlx::query(
        r#"
      INSERT INTO fetch_events(
        feed_id, event_time_ms, method,
        status, error_kind, latency_ms, backoff_index,
        scheduled_next_action_at_ms, debug
      ) VALUES (
        ?1, ?2, ?3,
        ?4, ?5, ?6, ?7,
        ?8, ?9
      )
      "#,
    )
    .bind(feed_id)
    .bind(now_ms)
    .bind(method)
    .bind(status)
    .bind(error_kind.map(|e| format!("{:?}", e)))
    .bind(latency_ms)
    .bind(backoff_index)
    .bind(scheduled_next_action_at_ms)
    .bind(debug.map(|s| s.to_string()))
    .execute(pool)
    .await
    .map_err(|e| format!("insert_event error: {e}"))?;
    Ok(())
}
