use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::domain::model::ErrorKind;
use crate::infra::time::epoch_ms_to_iso;

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
    zone: &Tz,
) -> Result<(), String> {
    let now_ms = now_epoch_ms();
    sqlx::query(
        r#"
      INSERT INTO fetch_events(
        feed_id, event_time_ms, event_time_text, method,
        status, error_kind, latency_ms, backoff_index,
        scheduled_next_action_at_ms, scheduled_next_action_at_text, debug
      ) VALUES (
        ?1, ?2, ?3, ?4,
        ?5, ?6, ?7, ?8,
        ?9, ?10, ?11
      )
      "#,
    )
    .bind(feed_id)
    .bind(now_ms)
    .bind(epoch_ms_to_iso(now_ms, zone))
    .bind(method)
    .bind(status)
    .bind(error_kind.map(|e| format!("{:?}", e)))
    .bind(latency_ms)
    .bind(backoff_index)
    .bind(scheduled_next_action_at_ms)
    .bind(epoch_ms_to_iso(scheduled_next_action_at_ms, zone))
    .bind(debug.map(|s| s.to_string()))
    .execute(pool)
    .await
    .map_err(|e| format!("insert_event error: {e}"))?;
    Ok(())
}
