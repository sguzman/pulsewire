//! Database migrations: create tables/indexes and ensure new columns exist.
use chrono_tz::Tz;
use sqlx::SqlitePool;
use tracing::info;

use super::connection::{
    ensure_feed_base_poll_column, ensure_feed_category_column, ensure_feed_state_error_count_column,
    ensure_feed_state_note_column,
};

const SQLITE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/res/sql/sqlite/schema.sql"
));

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

    // Ensure category exists before schema indexes use it on existing DBs.
    ensure_feed_category_column(pool).await?;

    for ddl in schema_statements() {
        sqlx::query(ddl)
            .execute(pool)
            .await
            .map_err(|e| format!("migrate error (ddl): {e}"))?;
    }

    ensure_feed_base_poll_column(pool, default_poll_seconds).await?;
    ensure_feed_state_note_column(pool).await?;
    ensure_feed_state_error_count_column(pool, "feed_state_current").await?;
    ensure_feed_state_error_count_column(pool, "feed_state_history").await?;

    info!("DB migrate done");
    Ok(())
}

fn schema_statements() -> impl Iterator<Item = &'static str> {
    SQLITE_SCHEMA
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
}
