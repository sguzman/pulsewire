//! Records feeds that exceeded the consecutive error threshold.
use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::domain::model::ErrorKind;

pub async fn mark_feed_error(
    pool: &SqlitePool,
    feed_id: &str,
    error_kind: Option<ErrorKind>,
    status: Option<i64>,
    error_count: i64,
    observed_at_ms: i64,
    _zone: &Tz,
) -> Result<(), String> {
    sqlx::query(
        r#"
      INSERT INTO error_feeds(
        feed_id, error_count, last_error_kind, last_error_status, last_error_at_ms, note
      ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6
      )
      ON CONFLICT(feed_id) DO UPDATE SET
        error_count = excluded.error_count,
        last_error_kind = excluded.last_error_kind,
        last_error_status = excluded.last_error_status,
        last_error_at_ms = excluded.last_error_at_ms,
        note = excluded.note
      "#,
    )
    .bind(feed_id)
    .bind(error_count)
    .bind(error_kind.map(|e| format!("{:?}", e)))
    .bind(status)
    .bind(observed_at_ms)
    .bind(Some("max-consecutive-errors".to_string()))
    .execute(pool)
    .await
    .map_err(|e| format!("mark_feed_error error: {e}"))?;
    Ok(())
}
