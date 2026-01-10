//! Inserts feed payload metadata and associated feed items in a single transaction (Postgres).
use chrono_tz::Tz;
use sqlx::PgPool;
use tracing::debug;

use super::util::{ts_from_ms, ts_from_ms_opt};
use crate::feed::parser::ParsedFeed;

pub async fn insert_payload_with_items(
    pool: &PgPool,
    feed_id: &str,
    fetched_at_ms: i64,
    etag: Option<&str>,
    last_modified_ms: Option<i64>,
    content_hash: Option<&str>,
    parsed: &ParsedFeed,
    zone: &Tz,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;
    let fetched_at = ts_from_ms(fetched_at_ms, zone);
    let last_modified_at = ts_from_ms_opt(last_modified_ms, zone);
    let updated_at = ts_from_ms_opt(parsed.metadata.updated_at_ms, zone);

    let payload_id: i64 = sqlx::query_scalar(
        r#"
      INSERT INTO feed_payloads(
        feed_id, fetched_at, etag,
        last_modified_at, content_hash,
        title, link, description, language,
        updated_at
      ) VALUES (
        $1, $2, $3,
        $4, $5,
        $6, $7, $8, $9,
        $10
      )
      RETURNING id;
      "#,
    )
    .bind(feed_id)
    .bind(fetched_at)
    .bind(etag.map(|s| s.to_string()))
    .bind(last_modified_at)
    .bind(content_hash.map(|s| s.to_string()))
    .bind(parsed.metadata.title.clone())
    .bind(parsed.metadata.link.clone())
    .bind(parsed.metadata.description.clone())
    .bind(parsed.metadata.language.clone())
    .bind(updated_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("insert payload: {e}"))?;

    for it in &parsed.items {
        sqlx::query(
            r#"
        INSERT INTO feed_items(
          payload_id, feed_id, title, link, guid,
          published_at,
          category, description, summary
        ) VALUES (
          $1, $2, $3, $4, $5,
          $6,
          $7, $8, $9
        )
        "#,
        )
        .bind(payload_id)
        .bind(feed_id)
        .bind(it.title.clone())
        .bind(it.link.clone())
        .bind(it.guid.clone())
        .bind(ts_from_ms_opt(it.published_at_ms, zone))
        .bind(it.category.clone())
        .bind(it.description.clone())
        .bind(it.summary.clone())
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("insert item: {e}"))?;
    }

    tx.commit().await.map_err(|e| format!("tx commit: {e}"))?;
    debug!(feed_id, payload_id, "Inserted payload + items");
    Ok(())
}
