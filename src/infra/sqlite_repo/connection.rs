//! Helpers to create/configure the SQLite pool and backfill missing columns.
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use tracing::info;

pub async fn create_pool(db_path: &Path) -> Result<SqlitePool, String> {
    let full_path = if db_path.is_absolute() {
        db_path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(db_path)
    };

    if let Some(parent) = full_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("db dir create error: {e}"))?;
    }

    let url = format!("sqlite://{}", full_path.display());
    let opts = SqliteConnectOptions::from_str(&url)
        .map_err(|e| format!("db connect options error: {e}"))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await
        .map_err(|e| format!("db connect error: {e}"))
}

pub async fn ensure_feed_base_poll_column(
    pool: &SqlitePool,
    default_poll_seconds: u64,
) -> Result<(), String> {
    let has_column: Option<i64> = sqlx::query_scalar(
        r#"SELECT 1 FROM pragma_table_info('feeds') WHERE name = 'base_poll_seconds' LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("introspect feeds table: {e}"))?;

    if has_column.is_some() {
        return Ok(());
    }

    let ddl = format!(
        "ALTER TABLE feeds ADD COLUMN base_poll_seconds INTEGER NOT NULL DEFAULT {}",
        default_poll_seconds as i64
    );
    sqlx::query(&ddl)
        .execute(pool)
        .await
        .map_err(|e| format!("add base_poll_seconds column: {e}"))?;
    info!(
        default_poll_seconds,
        "Backfilled base_poll_seconds on feeds"
    );
    Ok(())
}

pub async fn ensure_feed_state_note_column(pool: &SqlitePool) -> Result<(), String> {
    let has_column: Option<i64> = sqlx::query_scalar(
        r#"SELECT 1 FROM pragma_table_info('feed_state_current') WHERE name = 'note' LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("introspect feed_state_current: {e}"))?;

    if has_column.is_some() {
        return Ok(());
    }

    sqlx::query("ALTER TABLE feed_state_current ADD COLUMN note TEXT NULL")
        .execute(pool)
        .await
        .map_err(|e| format!("add note column: {e}"))?;
    info!("Added note column to feed_state_current");
    Ok(())
}

pub async fn ensure_feed_state_error_count_column(
    pool: &SqlitePool,
    table: &str,
) -> Result<(), String> {
    let sql = format!(
        "SELECT 1 FROM pragma_table_info('{table}') WHERE name = 'consecutive_error_count' LIMIT 1"
    );
    let has_column: Option<i64> = sqlx::query_scalar(&sql)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("introspect {table}: {e}"))?;

    if has_column.is_some() {
        return Ok(());
    }

    let ddl = format!(
        "ALTER TABLE {table} ADD COLUMN consecutive_error_count INTEGER NOT NULL DEFAULT 0"
    );
    sqlx::query(&ddl)
        .execute(pool)
        .await
        .map_err(|e| format!("add consecutive_error_count column: {e}"))?;
    info!(
        table,
        "Added consecutive_error_count column to feed state table"
    );
    Ok(())
}

pub async fn ensure_feed_category_column(pool: &SqlitePool) -> Result<(), String> {
    let has_table: Option<i64> = sqlx::query_scalar(
        r#"SELECT 1 FROM sqlite_master WHERE type='table' AND name='feeds' LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("introspect sqlite_master: {e}"))?;

    if has_table.is_none() {
        return Ok(());
    }

    let has_column: Option<i64> = sqlx::query_scalar(
        r#"SELECT 1 FROM pragma_table_info('feeds') WHERE name = 'category' LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("introspect feeds table: {e}"))?;

    if has_column.is_some() {
        return Ok(());
    }

    sqlx::query("ALTER TABLE feeds ADD COLUMN category TEXT NULL")
        .execute(pool)
        .await
        .map_err(|e| format!("add category column: {e}"))?;
    info!("Added category column to feeds");
    Ok(())
}

pub async fn set_synchronous(pool: &SqlitePool, mode: &str) -> Result<String, String> {
    let prev: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("read pragma synchronous: {e}"))?;
    sqlx::query(&format!("PRAGMA synchronous={mode}"))
        .execute(pool)
        .await
        .map_err(|e| format!("set pragma synchronous: {e}"))?;
    Ok(synchronous_to_string(prev))
}

pub fn synchronous_to_string(val: i64) -> String {
    match val {
        0 => "OFF",
        1 => "NORMAL",
        2 => "FULL",
        3 => "EXTRA",
        _ => "FULL",
    }
    .to_string()
}
