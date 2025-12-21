//! Records feeds that exceeded the consecutive error threshold in Postgres.
use chrono_tz::Tz;
use sqlx::PgPool;

use crate::domain::model::ErrorKind;

pub async fn mark_feed_error(
    pool: &PgPool,
    feed_id: &str,
    error_kind: Option<ErrorKind>,
    status: Option<i64>,
    error_count: i64,
    observed_at_ms: i64,
    zone: &Tz,
) -> Result<(), String> {
    let observed_at = super::util::ts_from_ms(observed_at_ms, zone);
    sqlx::query(
        r#"
      INSERT INTO error_feeds(
        feed_id, error_count, last_error_kind, last_error_status, last_error_at, note
      ) VALUES (
        $1, $2, $3, $4, $5, $6
      )
      ON CONFLICT(feed_id) DO UPDATE SET
        error_count = EXCLUDED.error_count,
        last_error_kind = EXCLUDED.last_error_kind,
        last_error_status = EXCLUDED.last_error_status,
        last_error_at = EXCLUDED.last_error_at,
        note = EXCLUDED.note
      "#,
    )
    .bind(feed_id)
    .bind(error_count)
    .bind(error_kind.map(|e| format!("{:?}", e)))
    .bind(status)
    .bind(observed_at)
    .bind(Some("max-consecutive-errors".to_string()))
    .execute(pool)
    .await
    .map_err(|e| format!("mark_feed_error error: {e}"))?;
    Ok(())
}
