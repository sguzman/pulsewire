//! Source cookie persistence for
//! SQLite-backed fetch runs.

use chrono_tz::Tz;
use sqlx::SqlitePool;

pub async fn latest_cookie_header(
  pool: &SqlitePool,
  feed_id: &str
) -> Result<Option<String>, String> {
  sqlx::query_scalar::<_, String>(
    r#"
      SELECT cookie_header
      FROM source_cookies
      WHERE feed_id = ?1
      LIMIT 1
      "#
  )
  .bind(feed_id)
  .fetch_optional(pool)
  .await
  .map_err(|e| {
    format!(
      "latest_cookie_header error: {e}"
    )
  })
}

pub async fn upsert_cookie_header(
  pool: &SqlitePool,
  feed_id: &str,
  cookie_header: &str,
  observed_at_ms: i64,
  _zone: &Tz
) -> Result<(), String> {
  sqlx::query(
    r#"
      INSERT INTO source_cookies(
        feed_id,
        cookie_header,
        updated_at_ms
      ) VALUES (?1, ?2, ?3)
      ON CONFLICT(feed_id)
      DO UPDATE SET
        cookie_header = excluded.cookie_header,
        updated_at_ms = excluded.updated_at_ms
      "#,
  )
  .bind(feed_id)
  .bind(cookie_header)
  .bind(observed_at_ms)
  .execute(pool)
  .await
  .map_err(|e| {
    format!(
      "upsert_cookie_header error: {e}"
    )
  })?;

  Ok(())
}
