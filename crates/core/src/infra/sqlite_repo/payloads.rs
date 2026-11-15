//! Inserts feed payload metadata and
//! associated feed items in a single
//! transaction.

use chrono_tz::Tz;
use sqlx::SqlitePool;
use tracing::debug;

use crate::feed::parser::ParsedFeed;
use crate::ports::repo::WatchObservation;

pub async fn latest_payload_content_hash(
  pool: &SqlitePool,
  feed_id: &str
) -> Result<Option<String>, String> {
  sqlx::query_scalar::<_, String>(
    r#"
      SELECT content_hash
      FROM feed_payloads
      WHERE feed_id = ?1
        AND content_hash IS NOT NULL
      ORDER BY id DESC
      LIMIT 1
    "#
  )
  .bind(feed_id)
  .fetch_optional(pool)
  .await
  .map_err(|e| {
    format!(
      "latest_payload_content_hash: \
       {e}"
    )
  })
}

pub async fn latest_watch_observation(
  pool: &SqlitePool,
  feed_id: &str
) -> Result<Option<WatchObservation>, String> {
  let value = sqlx::query_as::<
    _,
    (i64, Option<String>)
  >(
    r#"SELECT content_length,
       element_hash
       FROM watch_observations
       WHERE feed_id = ?1"#
  )
  .bind(feed_id)
  .fetch_optional(pool)
  .await
  .map_err(|e| {
    format!("read watch snapshot: {e}")
  })?;

  Ok(value.map(|(content_length, element_hash)| {
    WatchObservation {
      content_length,
      element_hash
    }
  }))
}

pub async fn upsert_watch_observation(
  pool: &SqlitePool,
  feed_id: &str,
  observation: &WatchObservation,
  observed_at_ms: i64
) -> Result<(), String> {
  sqlx::query(
    r#"INSERT INTO watch_observations(
         feed_id, content_length,
         element_hash, observed_at_ms)
       VALUES (?1, ?2, ?3, ?4)
       ON CONFLICT(feed_id) DO UPDATE
       SET content_length =
             excluded.content_length,
           element_hash =
             excluded.element_hash,
           observed_at_ms =
             excluded.observed_at_ms"#
  )
  .bind(feed_id)
  .bind(observation.content_length)
  .bind(observation.element_hash.as_deref())
  .bind(observed_at_ms)
  .execute(pool)
  .await
  .map_err(|e| {
    format!("save watch snapshot: {e}")
  })?;
  Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_payload_with_items(
  pool: &SqlitePool,
  feed_id: &str,
  fetched_at_ms: i64,
  etag: Option<&str>,
  last_modified_ms: Option<i64>,
  content_hash: Option<&str>,
  parsed: &ParsedFeed,
  _zone: &Tz
) -> Result<(), String> {
  let mut tx =
    pool.begin().await.map_err(
      |e| format!("tx begin: {e}")
    )?;

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

  tx.commit().await.map_err(|e| {
    format!("tx commit: {e}")
  })?;

  debug!(
    feed_id,
    payload_id,
    "Inserted payload + items"
  );

  Ok(())
}

#[cfg(test)]
mod persisted_hash_tests {
  use super::*;

  #[tokio::test]
  async fn latest_hash_is_feed_scoped_and_skips_nulls()
   {
    let pool = SqlitePool::connect(
      "sqlite::memory:"
    )
    .await
    .expect("in-memory sqlite");
    sqlx::query(
      "CREATE TABLE feed_payloads(id \
       INTEGER PRIMARY KEY,feed_id \
       TEXT NOT NULL,content_hash \
       TEXT NULL)"
    )
    .execute(&pool)
    .await
    .expect("table");

    for (feed, hash) in [
      ("first", Some("old")),
      ("second", Some("foreign")),
      ("first", Some("new")),
      ("first", None)
    ] {
      sqlx::query(
        "INSERT INTO \
         feed_payloads(feed_id, \
         content_hash) VALUES (?1, ?2)"
      )
      .bind(feed)
      .bind(hash)
      .execute(&pool)
      .await
      .expect("fixture insert");
    }

    assert_eq!(
      latest_payload_content_hash(
        &pool, "first"
      )
      .await
      .unwrap()
      .as_deref(),
      Some("new")
    );
    assert_eq!(
      latest_payload_content_hash(
        &pool, "second"
      )
      .await
      .unwrap()
      .as_deref(),
      Some("foreign")
    );
    assert_eq!(
      latest_payload_content_hash(
        &pool, "missing"
      )
      .await
      .unwrap(),
      None
    );
  }
}
