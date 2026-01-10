//! Records fetch events (HEAD/GET) with timing/status/error info for Postgres.
use chrono_tz::Tz;
use sqlx::PgPool;

use crate::domain::model::ErrorKind;

use super::util::{now_epoch_ms, ts_from_ms};

pub async fn insert_event(
    pool: &PgPool,
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
    let event_time = ts_from_ms(now_ms, zone);
    let scheduled_next_action_at = ts_from_ms(scheduled_next_action_at_ms, zone);
    sqlx::query(
        r#"
      INSERT INTO fetch_events(
        feed_id, event_time, method,
        status, error_kind, latency_ms, backoff_index,
        scheduled_next_action_at, debug
      ) VALUES (
        $1, $2, $3,
        $4, $5, $6, $7,
        $8, $9
      )
      "#,
    )
    .bind(feed_id)
    .bind(event_time)
    .bind(method)
    .bind(status)
    .bind(error_kind.map(|e| format!("{:?}", e)))
    .bind(latency_ms)
    .bind(backoff_index)
    .bind(scheduled_next_action_at)
    .bind(debug.map(|s| s.to_string()))
    .execute(pool)
    .await
    .map_err(|e| format!("insert_event error: {e}"))?;
    Ok(())
}
