//! Source cookie persistence for
//! Postgres-backed fetch runs.

use chrono_tz::Tz;
use sqlx::PgPool;

use super::util::ts_from_ms;

pub async fn latest_cookie_header(
  pool: &PgPool,
  feed_id: &str
) -> Result<Option<String>, String> {
  sqlx::query_scalar::<_, String>(
    r#"
      SELECT cookie_header
      FROM source_cookies
      WHERE feed_id = $1
      LIMIT 1
      "#,
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
  pool: &PgPool,
  feed_id: &str,
  cookie_header: &str,
  observed_at_ms: i64,
  zone: &Tz
) -> Result<(), String> {
  let updated_at =
    ts_from_ms(observed_at_ms, zone);

  sqlx::query(
    r#"
      INSERT INTO source_cookies(
        feed_id,
        cookie_header,
        updated_at
      ) VALUES ($1, $2, $3)
      ON CONFLICT(feed_id)
      DO UPDATE SET
        cookie_header = excluded.cookie_header,
        updated_at = excluded.updated_at
      "#,
  )
  .bind(feed_id)
  .bind(cookie_header)
  .bind(updated_at)
  .execute(pool)
  .await
  .map_err(|e| {
    format!(
      "upsert_cookie_header error: {e}"
    )
  })?;

  Ok(())
}
