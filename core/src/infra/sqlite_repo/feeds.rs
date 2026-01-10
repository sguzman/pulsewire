//! Feed definition persistence: bulk upsert and due-feed selection.
use std::time::Instant;

use chrono_tz::Tz;
use sqlx::SqlitePool;
use tracing::info;

use crate::domain::model::FeedConfig;

use super::connection::set_synchronous;
use super::models::DueFeedRow;
use super::util::now_epoch_ms;

pub async fn upsert_feeds_bulk(
    pool: &SqlitePool,
    feeds: Vec<FeedConfig>,
    chunk_size: usize,
    _zone: &Tz,
) -> Result<(), String> {
    let prev_sync = set_synchronous(pool, "NORMAL").await?;
    let res = do_upsert_chunks(pool, feeds, chunk_size.max(1)).await;
    let _ = set_synchronous(pool, &prev_sync).await;
    res
}

async fn do_upsert_chunks(
    pool: &SqlitePool,
    feeds: Vec<FeedConfig>,
    chunk_size: usize,
) -> Result<(), String> {
    let mut chunk = Vec::with_capacity(chunk_size);
    let mut total = 0usize;
    let mut iter = feeds.into_iter();
    let ingest_start = Instant::now();

    while let Some(feed) = iter.next() {
        chunk.push(feed);
        if chunk.len() == chunk_size {
            upsert_chunk(pool, &chunk).await?;
            total += chunk.len();
            chunk.clear();
        }
    }

    if !chunk.is_empty() {
        upsert_chunk(pool, &chunk).await?;
        total += chunk.len();
    }

    info!(
        total,
        elapsed_ms = ingest_start.elapsed().as_millis(),
        "Bulk feed upsert complete"
    );
    Ok(())
}

async fn upsert_chunk(pool: &SqlitePool, feeds: &[FeedConfig]) -> Result<(), String> {
    let start = Instant::now();
    let mut tx = pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;
    let now_ms = now_epoch_ms();

    for f in feeds {
        sqlx::query(
            r#"
        INSERT INTO feeds(id, url, domain, category, base_poll_seconds, created_at_ms)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
          url = excluded.url,
          domain = excluded.domain,
          category = excluded.category,
          base_poll_seconds = excluded.base_poll_seconds
        "#,
        )
        .bind(&f.id)
        .bind(&f.url)
        .bind(&f.domain)
        .bind(&f.category)
        .bind(f.base_poll_seconds as i64)
        .bind(now_ms)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("upsert feed error: {e}"))?;
    }

    tx.commit().await.map_err(|e| format!("tx commit: {e}"))?;
    info!(
        chunk = feeds.len(),
        elapsed_ms = start.elapsed().as_millis(),
        "Upserted feed chunk"
    );
    Ok(())
}

pub async fn due_feeds(
    pool: &SqlitePool,
    category: &str,
    now_ms: i64,
    limit: i64,
) -> Result<Vec<FeedConfig>, String> {
    let start = Instant::now();
    let rows = sqlx::query_as::<_, DueFeedRow>(
        r#"
      SELECT f.id, f.url, f.domain, f.category, f.base_poll_seconds
      FROM feeds f
      LEFT JOIN feed_state_current s ON s.feed_id = f.id
      LEFT JOIN error_feeds e ON e.feed_id = f.id
      WHERE f.category = ?1
        AND e.feed_id IS NULL
        AND (s.feed_id IS NULL OR s.next_action_at_ms <= ?2)
      ORDER BY COALESCE(s.next_action_at_ms, ?2)
      LIMIT ?3
      "#,
    )
    .bind(category)
    .bind(now_ms)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("due_feeds error: {e}"))?;

    let elapsed = start.elapsed();
    let feeds = rows.into_iter().map(FeedConfig::from).collect::<Vec<_>>();

    info!(
        category,
        limit,
        due = feeds.len(),
        elapsed_ms = elapsed.as_millis(),
        "due_feeds query"
    );
    Ok(feeds)
}

pub async fn upsert_categories(pool: &SqlitePool, names: &[String]) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;
    let now_ms = now_epoch_ms();

    for name in names {
        sqlx::query(
            r#"
        INSERT OR IGNORE INTO categories(name, created_at_ms)
        VALUES (?1, ?2)
        "#,
        )
        .bind(name)
        .bind(now_ms)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("upsert category error: {e}"))?;
    }

    tx.commit().await.map_err(|e| format!("tx commit: {e}"))?;
    Ok(())
}
