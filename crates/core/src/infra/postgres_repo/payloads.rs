//! Inserts feed payload metadata and
//! associated feed items in a single
//! transaction (Postgres).

use chrono_tz::Tz;
use sqlx::PgPool;
use tracing::debug;

use super::util::{
  ts_from_ms,
  ts_from_ms_opt
};
use crate::feed::parser::ParsedFeed;
use crate::ports::repo::WatchObservation;

pub async fn known_watch_item_guids(
  pool: &PgPool,
  feed_id: &str,
  candidates: &[String]
) -> Result<Vec<String>, String> {
  let mut found = Vec::new();
  for chunk in candidates.chunks(250) {
    if chunk.is_empty() {
      continue;
    }

    let mut builder =
      sqlx::QueryBuilder::<
        sqlx::Postgres
      >::new(
        "SELECT DISTINCT guid FROM \
         feed_items WHERE feed_id = "
      );
    builder.push_bind(feed_id);
    builder.push(" AND guid IN (");
    {
      let mut values =
        builder.separated(", ");
      for guid in chunk {
        values.push_bind(guid);
      }
      values.push_unseparated(")");
    }

    let mut batch = builder
      .build_query_scalar::<String>()
      .fetch_all(pool)
      .await
      .map_err(|e| {
        format!(
          "lookup watch GUIDs: {e}"
        )
      })?;
    found.append(&mut batch);
  }
  Ok(found)
}

pub async fn latest_payload_content_hash(
  pool: &PgPool,
  feed_id: &str
) -> Result<Option<String>, String> {
  sqlx::query_scalar::<_, String>(
    r#"
      SELECT content_hash
      FROM feed_payloads
      WHERE feed_id = $1
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
  pool: &PgPool,
  feed_id: &str
) -> Result<
  Option<WatchObservation>,
  String
> {
  let value = sqlx::query_as::<
    _,
    (i64, Option<String>)
  >(
    r#"SELECT content_length,
       element_hash
       FROM watch_observations
       WHERE feed_id = $1"#
  )
  .bind(feed_id)
  .fetch_optional(pool)
  .await
  .map_err(|e| {
    format!("read watch snapshot: {e}")
  })?;

  Ok(value.map(
    |(content_length, element_hash)| {
      WatchObservation {
        content_length,
        element_hash
      }
    }
  ))
}

pub async fn upsert_watch_observation(
  pool: &PgPool,
  feed_id: &str,
  observation: &WatchObservation,
  observed_at_ms: i64
) -> Result<(), String> {
  sqlx::query(
    r#"INSERT INTO watch_observations(
         feed_id, content_length,
         element_hash, observed_at_ms)
       VALUES ($1, $2, $3, $4)
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
  .bind(
    observation.element_hash.as_deref()
  )
  .bind(observed_at_ms)
  .execute(pool)
  .await
  .map_err(|e| {
    format!("save watch snapshot: {e}")
  })?;
  Ok(())
}

pub async fn latest_watch_emitted_count(
  pool: &PgPool,
  feed_id: &str
) -> Result<Option<i64>, String> {
  sqlx::query_scalar::<_, i64>(
    "SELECT item_count FROM \
     watch_emit_watermarks \
     WHERE feed_id = $1"
  )
  .bind(feed_id)
  .fetch_optional(pool)
  .await
  .map_err(|e| {
    format!("read watch emitted count: {e}")
  })
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_payload_with_items(
  pool: &PgPool,
  feed_id: &str,
  fetched_at_ms: i64,
  etag: Option<&str>,
  last_modified_ms: Option<i64>,
  content_hash: Option<&str>,
  parsed: &ParsedFeed,
  watch_item_count: Option<i64>,
  zone: &Tz
) -> Result<(), String> {
  let mut tx =
    pool.begin().await.map_err(
      |e| format!("tx begin: {e}")
    )?;

  let fetched_at =
    ts_from_ms(fetched_at_ms, zone);

  let last_modified_at = ts_from_ms_opt(
    last_modified_ms,
    zone
  );

  let updated_at = ts_from_ms_opt(
    parsed.metadata.updated_at_ms,
    zone
  );

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

  // Count baseline advances only when
  // the emitted payload commits.
  if let Some(count) = watch_item_count {
    sqlx::query(
      r#"INSERT INTO watch_emit_watermarks(
           feed_id, item_count,
           emitted_at_ms)
         VALUES ($1, $2, $3)
         ON CONFLICT(feed_id) DO UPDATE
         SET item_count = excluded.item_count,
             emitted_at_ms = excluded.emitted_at_ms"#
    )
    .bind(feed_id)
    .bind(count)
    .bind(fetched_at_ms)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
      format!("save emitted count: {e}")
    })?;
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
mod postgres_watch_emission_tests {
  use super::*;
  use crate::feed::parser::{
    FeedItem,
    FeedMetadata
  };

  #[tokio::test]
  #[ignore = "requires disposable PostgreSQL CI service"]
  async fn postgres_watch_emission_roundtrip()
   {
    let password = std::env::var(
      "PULSEWIRE_TEST_POSTGRES_PASSWORD"
    )
    .expect("CI PostgreSQL password");

    let options =
      sqlx::postgres::PgConnectOptions::new()
        .host("127.0.0.1")
        .port(5432)
        .username("pulsewire_ci")
        .password(&password)
        .database("pulsewire_ci");

    let nonce = std::time::SystemTime::now()
      .duration_since(
        std::time::UNIX_EPOCH
      )
      .expect("system time")
      .as_nanos();
    let schema = format!(
      "pw_watch_emit_{}_{nonce}",
      std::process::id()
    );

    let first =
      sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .expect("disposable PostgreSQL");

    sqlx::query(&format!(
      "CREATE SCHEMA {schema}"
    ))
    .execute(&first)
    .await
    .expect("isolated schema");
    sqlx::query(&format!(
      "SET search_path TO {schema}"
    ))
    .execute(&first)
    .await
    .expect("use isolated schema");

    for ddl in [
      "CREATE TABLE feed_payloads(\
       id BIGSERIAL PRIMARY KEY, \
       feed_id TEXT, fetched_at TIMESTAMPTZ, \
       etag TEXT, last_modified_at TIMESTAMPTZ, \
       content_hash TEXT, title TEXT, \
       link TEXT, description TEXT, \
       language TEXT, updated_at TIMESTAMPTZ)",
      "CREATE TABLE feed_items(\
       payload_id BIGINT, feed_id TEXT, \
       title TEXT, link TEXT, guid TEXT, \
       published_at TIMESTAMPTZ, \
       category TEXT, description TEXT, \
       summary TEXT)"
    ] {
      sqlx::query(ddl)
        .execute(&first)
        .await
        .expect("fixture schema");
    }

    let parsed = ParsedFeed {
      metadata: FeedMetadata {
        title: Some("Page".into()),
        link: None,
        description: None,
        language: None,
        updated_at_ms: None
      },
      items: vec![FeedItem {
        title: Some("Story".into()),
        link: None,
        guid: Some("watch:story-1".into()),
        published_at_ms: None,
        category: None,
        description: None,
        summary: None
      }]
    };

    // Missing watermark table must
    // roll back payload insertion.
    assert!(
      insert_payload_with_items(
        &first, "watch", 1000,
        None, None, Some("hash"),
        &parsed, Some(4),
        &chrono_tz::UTC
      )
      .await
      .is_err()
    );
    let rows: i64 = sqlx::query_scalar(
      "SELECT COUNT(*) FROM feed_payloads"
    )
    .fetch_one(&first)
    .await
    .unwrap();
    assert_eq!(rows, 0);

    sqlx::query(
      "CREATE TABLE watch_emit_watermarks(\
       feed_id TEXT PRIMARY KEY, \
       item_count BIGINT NOT NULL, \
       emitted_at_ms BIGINT NOT NULL)"
    )
    .execute(&first)
    .await
    .expect("watermark schema");

    insert_payload_with_items(
      &first, "watch", 1000,
      None, None, Some("hash"),
      &parsed, Some(4),
      &chrono_tz::UTC
    )
    .await
    .expect("atomic postgres emission");

    first.close().await;

    // A different pool verifies
    // cross-connection durability.
    let second =
      sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("new PostgreSQL pool");
    sqlx::query(&format!(
      "SET search_path TO {schema}"
    ))
    .execute(&second)
    .await
    .expect("restore isolated schema");

    assert_eq!(
      latest_watch_emitted_count(
        &second, "watch"
      )
      .await
      .unwrap(),
      Some(4)
    );
    assert_eq!(
      known_watch_item_guids(
        &second,
        "watch",
        &["watch:story-1".into()]
      )
      .await
      .unwrap(),
      vec!["watch:story-1".to_string()]
    );

    sqlx::query(&format!(
      "DROP SCHEMA {schema} CASCADE"
    ))
    .execute(&second)
    .await
    .expect("remove disposable schema");
    second.close().await;
  }
}
