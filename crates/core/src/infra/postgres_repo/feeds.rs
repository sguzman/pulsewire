//! Feed definition persistence: bulk
//! upsert and due-feed selection for
//! Postgres.

use std::time::Instant;

use chrono_tz::Tz;
use sqlx::PgPool;
use tracing::info;

use super::models::DueFeedRow;
use super::util::now_epoch_ms;
use crate::domain::model::FeedConfig;

pub async fn upsert_feeds_bulk(
  pool: &PgPool,
  feeds: Vec<FeedConfig>,
  chunk_size: usize,
  zone: &Tz
) -> Result<(), String> {
  do_upsert_chunks(
    pool,
    feeds,
    chunk_size.max(1),
    zone
  )
  .await
}

async fn do_upsert_chunks(
  pool: &PgPool,
  feeds: Vec<FeedConfig>,
  chunk_size: usize,
  zone: &Tz
) -> Result<(), String> {
  let mut chunk =
    Vec::with_capacity(chunk_size);

  let mut total = 0usize;

  let iter = feeds.into_iter();

  let ingest_start = Instant::now();

  for feed in iter {
    chunk.push(feed);

    if chunk.len() == chunk_size {
      upsert_chunk(pool, &chunk, zone)
        .await?;

      total += chunk.len();

      chunk.clear();
    }
  }

  if !chunk.is_empty() {
    upsert_chunk(pool, &chunk, zone)
      .await?;

    total += chunk.len();
  }

  info!(
    total,
    elapsed_ms = ingest_start
      .elapsed()
      .as_millis(),
    "Bulk feed upsert complete"
  );

  Ok(())
}

async fn upsert_chunk(
  pool: &PgPool,
  feeds: &[FeedConfig],
  zone: &Tz
) -> Result<(), String> {
  let start = Instant::now();

  let mut tx =
    pool.begin().await.map_err(
      |e| format!("tx begin: {e}")
    )?;

  let now_ms = now_epoch_ms();

  let now_ts = super::util::ts_from_ms(
    now_ms, zone
  );

  for f in feeds {
    sqlx::query(
            r#"
        INSERT INTO feeds(id, url, domain, category, base_poll_seconds, tags, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (id) DO UPDATE SET
          url = EXCLUDED.url,
          domain = EXCLUDED.domain,
          category = EXCLUDED.category,
          base_poll_seconds = EXCLUDED.base_poll_seconds,
          tags = EXCLUDED.tags
        "#,
        )
        .bind(&f.id)
        .bind(&f.url)
        .bind(&f.domain)
        .bind(&f.category)
        .bind(f.base_poll_seconds as i64)
        .bind(f.tags.clone())
        .bind(now_ts)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("upsert feed error: {e}"))?;
  }

  tx.commit().await.map_err(|e| {
    format!("tx commit: {e}")
  })?;

  info!(
    chunk = feeds.len(),
    elapsed_ms =
      start.elapsed().as_millis(),
    "Upserted feed chunk"
  );

  Ok(())
}

pub async fn due_feeds(
  pool: &PgPool,
  category: &str,
  now_ms: i64,
  limit: i64,
  zone: &Tz
) -> Result<Vec<FeedConfig>, String> {
  let start = Instant::now();

  let now_ts = super::util::ts_from_ms(
    now_ms, zone
  );

  let rows = sqlx::query_as::<_, DueFeedRow>(
        r#"
      SELECT f.id, f.url, f.domain, f.category, f.base_poll_seconds
      FROM feeds f
      LEFT JOIN feed_state_current s ON s.feed_id = f.id
      LEFT JOIN error_feeds e ON e.feed_id = f.id
      WHERE f.category = $1
        AND e.feed_id IS NULL
        AND (s.feed_id IS NULL OR s.next_action_at <= $2)
      ORDER BY COALESCE(s.next_action_at, $2), f.id
      LIMIT $3
      "#,
    )
    .bind(category)
    .bind(now_ts)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("due_feeds error: {e}"))?;

  let elapsed = start.elapsed();

  let feeds = rows
    .into_iter()
    .map(FeedConfig::from)
    .collect::<Vec<_>>();

  info!(
    category,
    limit,
    due = feeds.len(),
    elapsed_ms = elapsed.as_millis(),
    "due_feeds query"
  );

  Ok(feeds)
}

pub async fn upsert_categories(
  pool: &PgPool,
  names: &[String],
  zone: &Tz
) -> Result<(), String> {
  let mut tx =
    pool.begin().await.map_err(
      |e| format!("tx begin: {e}")
    )?;

  let now_ms = now_epoch_ms();

  let now_ts = super::util::ts_from_ms(
    now_ms, zone
  );

  for name in names {
    sqlx::query(
            r#"
        INSERT INTO categories(name, created_at)
        VALUES ($1, $2)
        ON CONFLICT (name) DO NOTHING
        "#,
        )
        .bind(name)
        .bind(now_ts)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("upsert category error: {e}"))?;
  }

  tx.commit().await.map_err(|e| {
    format!("tx commit: {e}")
  })?;

  Ok(())
}

#[cfg(test)]
mod postgres_due_queue_tests {
  use super::*;

  // Always isolated: TEMP tables on the
  // CI service connection, not
  // the application's persistent
  // tables, schema, or real user data.
  #[tokio::test]
  #[ignore = "requires disposable \
              PostgreSQL CI service"]
  async fn postgres_due_queue_paging_and_errors()
   {
    let password = std::env::var(
      "PULSEWIRE_TEST_POSTGRES_PASSWORD"
    )
    .expect("CI-only PostgreSQL test password required");

    let options = sqlx::postgres::PgConnectOptions::new()
      .host("127.0.0.1")
      .port(5432)
      .username("pulsewire_ci")
      .password(&password)
      .database("pulsewire_ci");

    // One pooled connection preserves
    // this session's TEMP tables.
    let pool = sqlx::postgres::PgPoolOptions::new()
      .max_connections(1)
      .connect_with(options)
      .await
      .expect("connect to disposable CI PostgreSQL");

    for ddl in [
      "CREATE TEMP TABLE feeds (id \
       TEXT PRIMARY KEY, url TEXT NOT \
       NULL, domain TEXT NOT NULL, \
       category TEXT NOT NULL, \
       base_poll_seconds BIGINT NOT \
       NULL)",
      "CREATE TEMP TABLE \
       feed_state_current (feed_id \
       TEXT PRIMARY KEY, \
       next_action_at TIMESTAMPTZ NOT \
       NULL)",
      "CREATE TEMP TABLE error_feeds \
       (feed_id TEXT PRIMARY KEY)"
    ] {
      sqlx::query(ddl)
        .execute(&pool)
        .await
        .expect(
          "create isolated queue \
           fixture table"
        );
    }

    // Reverse insertion prevents
    // incidental insertion order from
    // making an unstable query appear
    // deterministic.
    sqlx::query(
      "INSERT INTO feeds (id, url, domain, category, base_poll_seconds)
       SELECT 'feed-' || LPAD(n::text, 4, '0'),
              'https://example.test/' || n, 'example.test', 'news', 60
       FROM generate_series(1101, 1, -1) AS generated(n)"
    )
    .execute(&pool)
    .await
    .expect("populate 1,101 news feed fixtures");

    sqlx::query(
      "INSERT INTO feeds (id, url, domain, category, base_poll_seconds)
       VALUES ('other-1', 'https://example.test/other-1',
               'example.test', 'other', 60)"
    )
    .execute(&pool)
    .await
    .expect("populate unrelated category");

    let now_ms: i64 = 1_700_000_000_000;
    let zone = chrono_tz::UTC;
    let first = due_feeds(
      &pool, "news", now_ms, 1000,
      &zone
    )
    .await
    .expect("first bounded due query");
    assert_eq!(first.len(), 1000);
    assert_eq!(
      first
        .first()
        .map(|f| f.id.as_str()),
      Some("feed-0001")
    );
    assert_eq!(
      first
        .last()
        .map(|f| f.id.as_str()),
      Some("feed-1000")
    );
    assert!(
      first
        .windows(2)
        .all(|w| w[0].id < w[1].id)
    );

    // Emulate finished requests
    // scheduling their next action
    // later.
    sqlx::query(
      "INSERT INTO feed_state_current \
       (feed_id, next_action_at)
       SELECT id, \
       to_timestamp($1::double \
       precision)
       FROM feeds WHERE category = \
       'news' AND id <= 'feed-1000'"
    )
    .bind(1_700_003_600.0f64)
    .execute(&pool)
    .await
    .expect(
      "defer processed first page"
    );

    sqlx::query(
      "INSERT INTO error_feeds \
       (feed_id) VALUES ('feed-1050')"
    )
    .execute(&pool)
    .await
    .expect(
      "mark a subsequent feed errored"
    );

    let second = due_feeds(
      &pool, "news", now_ms, 1000,
      &zone
    )
    .await
    .expect(
      "remaining due feeds after \
       first page"
    );
    let actual: Vec<_> = second
      .iter()
      .map(|f| f.id.clone())
      .collect();
    let expected: Vec<_> = (1001
      ..=1101)
      .filter(|n| *n != 1050)
      .map(|n| format!("feed-{n:04}"))
      .collect();
    assert_eq!(actual, expected);
    assert_eq!(second.len(), 100);

    let other = due_feeds(
      &pool, "other", now_ms, 1000,
      &zone
    )
    .await
    .expect(
      "unrelated category stays \
       available"
    );
    assert_eq!(other.len(), 1);
    assert_eq!(other[0].id, "other-1");

    let zero = due_feeds(
      &pool, "news", now_ms, 0, &zone
    )
    .await
    .expect("zero limit is bounded");
    assert!(zero.is_empty());
  }
}
