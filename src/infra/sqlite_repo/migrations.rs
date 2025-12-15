//! Database migrations: create tables/indexes and ensure new columns exist.
use chrono_tz::Tz;
use sqlx::SqlitePool;
use tracing::info;

use super::connection::{ensure_feed_base_poll_column, ensure_feed_state_note_column};

pub async fn migrate(
    pool: &SqlitePool,
    _zone: &Tz,
    default_poll_seconds: u64,
) -> Result<(), String> {
    info!("DB migrate start");

    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(pool)
        .await
        .map_err(|e| format!("migrate error (pragma): {e}"))?;

    let ddls = [
        r#"
      CREATE TABLE IF NOT EXISTS feeds(
        id TEXT PRIMARY KEY,
        url TEXT NOT NULL,
        domain TEXT NOT NULL,
        base_poll_seconds INTEGER NOT NULL,
        created_at_ms INTEGER NOT NULL,
        created_at_text TEXT NOT NULL
      )"#,
        r#"
      CREATE TABLE IF NOT EXISTS feed_state_history(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        feed_id TEXT NOT NULL REFERENCES feeds(id),
        recorded_at_ms INTEGER NOT NULL,
        recorded_at_text TEXT NOT NULL,
        phase TEXT NOT NULL,
        last_head_at_ms INTEGER NULL,
        last_head_at_text TEXT NULL,
        last_head_status INTEGER NULL,
        last_head_error TEXT NULL,
        last_get_at_ms INTEGER NULL,
        last_get_at_text TEXT NULL,
        last_get_status INTEGER NULL,
        last_get_error TEXT NULL,
        etag TEXT NULL,
        last_modified_ms INTEGER NULL,
        last_modified_text TEXT NULL,
        backoff_index INTEGER NOT NULL,
        base_poll_seconds INTEGER NOT NULL,
        next_action_at_ms INTEGER NOT NULL,
        next_action_at_text TEXT NOT NULL,
        jitter_seconds INTEGER NOT NULL,
        note TEXT NULL
      )"#,
        r#"
      CREATE TABLE IF NOT EXISTS fetch_events(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        feed_id TEXT NOT NULL REFERENCES feeds(id),
        event_time_ms INTEGER NOT NULL,
        event_time_text TEXT NOT NULL,
        method TEXT NOT NULL,
        status INTEGER NULL,
        error_kind TEXT NULL,
        latency_ms INTEGER NULL,
        backoff_index INTEGER NOT NULL,
        scheduled_next_action_at_ms INTEGER NOT NULL,
        scheduled_next_action_at_text TEXT NOT NULL,
        debug TEXT NULL
      )"#,
        r#"
      CREATE TABLE IF NOT EXISTS feed_payloads(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        feed_id TEXT NOT NULL REFERENCES feeds(id),
        fetched_at_ms INTEGER NOT NULL,
        fetched_at_text TEXT NOT NULL,
        etag TEXT NULL,
        last_modified_ms INTEGER NULL,
        last_modified_text TEXT NULL,
        content_hash TEXT NULL,
        title TEXT NULL,
        link TEXT NULL,
        description TEXT NULL,
        language TEXT NULL,
        updated_at_ms INTEGER NULL,
        updated_at_text TEXT NULL
      )"#,
        r#"
      CREATE TABLE IF NOT EXISTS feed_state_current(
        feed_id TEXT PRIMARY KEY REFERENCES feeds(id),
        phase TEXT NOT NULL,
        last_head_at_ms INTEGER NULL,
        last_head_at_text TEXT NULL,
        last_head_status INTEGER NULL,
        last_head_error TEXT NULL,
        last_get_at_ms INTEGER NULL,
        last_get_at_text TEXT NULL,
        last_get_status INTEGER NULL,
        last_get_error TEXT NULL,
        etag TEXT NULL,
        last_modified_ms INTEGER NULL,
        last_modified_text TEXT NULL,
        backoff_index INTEGER NOT NULL,
        base_poll_seconds INTEGER NOT NULL,
        next_action_at_ms INTEGER NOT NULL,
        next_action_at_text TEXT NOT NULL,
        jitter_seconds INTEGER NOT NULL,
        note TEXT NULL
      )"#,
        r#"
      CREATE INDEX IF NOT EXISTS idx_feed_state_current_next_action
      ON feed_state_current(next_action_at_ms)"#,
        r#"
      CREATE TABLE IF NOT EXISTS feed_items(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        payload_id INTEGER NOT NULL REFERENCES feed_payloads(id) ON DELETE CASCADE,
        feed_id TEXT NOT NULL REFERENCES feeds(id),
        title TEXT NULL,
        link TEXT NULL,
        guid TEXT NULL,
        published_at_ms INTEGER NULL,
        published_at_text TEXT NULL,
        category TEXT NULL,
        description TEXT NULL,
        summary TEXT NULL
      )"#,
        r#"CREATE INDEX IF NOT EXISTS idx_feed_items_payload ON feed_items(payload_id)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_feed_items_feed ON feed_items(feed_id)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_feeds_domain ON feeds(domain)"#,
    ];

    for ddl in ddls {
        sqlx::query(ddl)
            .execute(pool)
            .await
            .map_err(|e| format!("migrate error (ddl): {e}"))?;
    }

    ensure_feed_base_poll_column(pool, default_poll_seconds).await?;
    ensure_feed_state_note_column(pool).await?;

    info!("DB migrate done");
    Ok(())
}
