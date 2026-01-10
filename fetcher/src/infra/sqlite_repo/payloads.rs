//! Inserts feed payload metadata and associated feed items in a single transaction.
use chrono_tz::Tz;
use sqlx::SqlitePool;
use tracing::debug;

use crate::feed::parser::ParsedFeed;

pub async fn insert_payload_with_items(
    pool: &SqlitePool,
    feed_id: &str,
    fetched_at_ms: i64,
    etag: Option<&str>,
    last_modified_ms: Option<i64>,
    content_hash: Option<&str>,
    parsed: &ParsedFeed,
    _zone: &Tz,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;

    let payload_id: i64 = sqlx::query_scalar(
        r#"
  INSERT INTO feed_payloads(
        feed_id, fetched_at_ms, etag,
        last_modified_ms, content_hash,
        title, link, description, language,
        updated_at_ms
      ) VALUES (
        ?1, ?2, ?3,
        ?4, ?5,
        ?6, ?7, ?8, ?9,
        ?10
      );
      SELECT last_insert_rowid();
      "#,
    )
    .bind(feed_id)
    .bind(fetched_at_ms)
    .bind(etag.map(|s| s.to_string()))
    .bind(last_modified_ms)
    .bind(content_hash.map(|s| s.to_string()))
    .bind(parsed.metadata.title.clone())
    .bind(parsed.metadata.link.clone())
    .bind(parsed.metadata.description.clone())
    .bind(parsed.metadata.language.clone())
    .bind(parsed.metadata.updated_at_ms)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("insert payload: {e}"))?;

    for it in &parsed.items {
        sqlx::query(
            r#"
        INSERT INTO feed_items(
          payload_id, feed_id, title, link, guid,
          published_at_ms,
          category, description, summary
        ) VALUES (
          ?1, ?2, ?3, ?4, ?5,
          ?6,
          ?7, ?8, ?9
        )
        "#,
        )
        .bind(payload_id)
        .bind(feed_id)
        .bind(it.title.clone())
        .bind(it.link.clone())
        .bind(it.guid.clone())
        .bind(it.published_at_ms)
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
