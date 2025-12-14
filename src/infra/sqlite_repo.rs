use std::collections::HashMap;
use std::path::Path;

use chrono_tz::Tz;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tracing::{debug, info};

use crate::domain::link_state::LinkState;
use crate::domain::model::{ErrorKind, FeedConfig};
use crate::feed::parser::ParsedFeed;
use crate::infra::time::{epoch_ms_to_iso};
use crate::ports::repo::{Repo, StateRow};

#[derive(Debug, sqlx::FromRow)]
struct StateRowRecord {
  pub feed_id: String,
  pub phase: String,
  pub last_head_at_ms: Option<i64>,
  pub last_head_status: Option<i64>,
  pub last_head_error: Option<String>,
  pub last_get_at_ms: Option<i64>,
  pub last_get_status: Option<i64>,
  pub last_get_error: Option<String>,
  pub etag: Option<String>,
  pub last_modified_ms: Option<i64>,
  pub backoff_index: i64,
  pub base_poll_seconds: i64,
  pub next_action_at_ms: i64,
  pub jitter_seconds: i64,
  pub note: Option<String>,
}

impl From<StateRowRecord> for StateRow {
  fn from(value: StateRowRecord) -> Self {
    Self {
      feed_id: value.feed_id,
      phase: value.phase,
      last_head_at_ms: value.last_head_at_ms,
      last_head_status: value.last_head_status,
      last_head_error: value.last_head_error,
      last_get_at_ms: value.last_get_at_ms,
      last_get_status: value.last_get_status,
      last_get_error: value.last_get_error,
      etag: value.etag,
      last_modified_ms: value.last_modified_ms,
      backoff_index: value.backoff_index,
      base_poll_seconds: value.base_poll_seconds,
      next_action_at_ms: value.next_action_at_ms,
      jitter_seconds: value.jitter_seconds,
      note: value.note,
    }
  }
}

pub struct SqliteRepo {
  pool: SqlitePool,
}

impl SqliteRepo {
  pub async fn new(db_path: &Path) -> Result<Self, String> {
    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
      std::fs::create_dir_all(parent).map_err(|e| format!("db dir create error: {e}"))?;
    }
    let url = format!("sqlite://{}", db_path.display());
    let pool = SqlitePoolOptions::new()
      .max_connections(10)
      .connect(&url)
      .await
      .map_err(|e| format!("db connect error: {e}"))?;
    Ok(Self { pool })
  }
}

#[async_trait::async_trait]
impl Repo for SqliteRepo {
  async fn migrate(&self, _zone: &Tz) -> Result<(), String> {
    info!("DB migrate start");

    // PRAGMA WAL
    sqlx::query("PRAGMA journal_mode=WAL")
      .execute(&self.pool)
      .await
      .map_err(|e| format!("migrate error (pragma): {e}"))?;

    // Tables closely match your Scala schema. :contentReference[oaicite:4]{index=4}
    let ddls = [
      r#"
      CREATE TABLE IF NOT EXISTS feeds(
        id TEXT PRIMARY KEY,
        url TEXT NOT NULL,
        domain TEXT NOT NULL,
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
        jitter_seconds INTEGER NOT NULL
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
    ];

    for ddl in ddls {
      sqlx::query(ddl)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("migrate error (ddl): {e}"))?;
    }

    info!("DB migrate done");
    Ok(())
  }

  async fn upsert_feeds(&self, feeds: &[FeedConfig], zone: &Tz) -> Result<(), String> {
    let now_ms = now_epoch_ms();
    for f in feeds {
      sqlx::query(
        r#"
        INSERT OR IGNORE INTO feeds(id, url, domain, created_at_ms, created_at_text)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
      )
      .bind(&f.id)
      .bind(&f.url)
      .bind(&f.domain)
      .bind(now_ms)
      .bind(epoch_ms_to_iso(now_ms, zone))
      .execute(&self.pool)
      .await
      .map_err(|e| format!("upsert feed error: {e}"))?;
    }
    Ok(())
  }

  async fn latest_state(&self, feed_id: &str) -> Result<Option<StateRow>, String> {
    let row = sqlx::query_as::<_, StateRowRecord>(
      r#"
      SELECT
        feed_id,
        phase,
        last_head_at_ms,
        last_head_status,
        last_head_error,
        last_get_at_ms,
        last_get_status,
        last_get_error,
        etag,
        last_modified_ms,
        backoff_index,
        base_poll_seconds,
        next_action_at_ms,
        jitter_seconds,
        note
      FROM feed_state_history
      WHERE feed_id = ?1
      ORDER BY id DESC
      LIMIT 1
      "#,
    )
    .bind(feed_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| format!("latest_state error: {e}"))?;
    Ok(row.map(StateRow::from))
  }

  async fn due_feeds(&self, now_ms: i64, feeds: &[FeedConfig], limit: i64) -> Result<Vec<FeedConfig>, String> {
    let mut feed_map: HashMap<&str, &FeedConfig> = HashMap::new();
    for f in feeds {
      feed_map.insert(f.id.as_str(), f);
    }

    let ids = sqlx::query_scalar::<_, String>(
      r#"
      SELECT f.id
      FROM feeds f
      LEFT JOIN feed_state_current s ON s.feed_id = f.id
      WHERE s.feed_id IS NULL OR s.next_action_at_ms <= ?1
      ORDER BY COALESCE(s.next_action_at_ms, strftime('%s','now')*1000)
      LIMIT ?2
      "#,
    )
    .bind(now_ms)
    .bind(limit)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| format!("due_feeds error: {e}"))?;

    let mut out = Vec::new();
    for id in ids {
      if let Some(f) = feed_map.get(id.as_str()) {
        out.push((*f).clone());
      }
    }
    Ok(out)
  }

  async fn insert_state(&self, state: &LinkState, recorded_at_ms: i64, zone: &Tz) -> Result<(), String> {
    let rec_text = epoch_ms_to_iso(recorded_at_ms, zone);

    let next_text = epoch_ms_to_iso(state.next_action_at_ms, zone);

    // History
    sqlx::query(
      r#"
      INSERT INTO feed_state_history(
        feed_id, recorded_at_ms, recorded_at_text, phase,
        last_head_at_ms, last_head_at_text, last_head_status, last_head_error,
        last_get_at_ms, last_get_at_text, last_get_status, last_get_error,
        etag, last_modified_ms, last_modified_text,
        backoff_index, base_poll_seconds, next_action_at_ms, next_action_at_text,
        jitter_seconds, note
      ) VALUES (
        ?1, ?2, ?3, ?4,
        ?5, ?6, ?7, ?8,
        ?9, ?10, ?11, ?12,
        ?13, ?14, ?15,
        ?16, ?17, ?18, ?19,
        ?20, ?21
      )
      "#,
    )
    .bind(&state.feed_id)
    .bind(recorded_at_ms)
    .bind(&rec_text)
    .bind(format!("{:?}", state.phase))
    .bind(state.last_head_at_ms)
    .bind(state.last_head_at_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .bind(state.last_head_status.map(|x| x as i64))
    .bind(state.last_head_error.map(|e| format!("{:?}", e)))
    .bind(state.last_get_at_ms)
    .bind(state.last_get_at_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .bind(state.last_get_status.map(|x| x as i64))
    .bind(state.last_get_error.map(|e| format!("{:?}", e)))
    .bind(&state.etag)
    .bind(state.last_modified_ms)
    .bind(state.last_modified_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .bind(state.backoff_index as i64)
    .bind(state.base_poll_seconds as i64)
    .bind(state.next_action_at_ms)
    .bind(&next_text)
    .bind(state.jitter_seconds)
    .bind(&state.note)
    .execute(&self.pool)
    .await
    .map_err(|e| format!("insert_state history error: {e}"))?;

    // Current upsert
    sqlx::query(
      r#"
      INSERT INTO feed_state_current(
        feed_id, phase,
        last_head_at_ms, last_head_at_text, last_head_status, last_head_error,
        last_get_at_ms, last_get_at_text, last_get_status, last_get_error,
        etag, last_modified_ms, last_modified_text,
        backoff_index, base_poll_seconds, next_action_at_ms, next_action_at_text,
        jitter_seconds
      ) VALUES (
        ?1, ?2,
        ?3, ?4, ?5, ?6,
        ?7, ?8, ?9, ?10,
        ?11, ?12, ?13,
        ?14, ?15, ?16, ?17,
        ?18
      )
      ON CONFLICT(feed_id) DO UPDATE SET
        phase = excluded.phase,
        last_head_at_ms = excluded.last_head_at_ms,
        last_head_at_text = excluded.last_head_at_text,
        last_head_status = excluded.last_head_status,
        last_head_error = excluded.last_head_error,
        last_get_at_ms = excluded.last_get_at_ms,
        last_get_at_text = excluded.last_get_at_text,
        last_get_status = excluded.last_get_status,
        last_get_error = excluded.last_get_error,
        etag = excluded.etag,
        last_modified_ms = excluded.last_modified_ms,
        last_modified_text = excluded.last_modified_text,
        backoff_index = excluded.backoff_index,
        base_poll_seconds = excluded.base_poll_seconds,
        next_action_at_ms = excluded.next_action_at_ms,
        next_action_at_text = excluded.next_action_at_text,
        jitter_seconds = excluded.jitter_seconds
      "#,
    )
    .bind(&state.feed_id)
    .bind(format!("{:?}", state.phase))
    .bind(state.last_head_at_ms)
    .bind(state.last_head_at_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .bind(state.last_head_status.map(|x| x as i64))
    .bind(state.last_head_error.map(|e| format!("{:?}", e)))
    .bind(state.last_get_at_ms)
    .bind(state.last_get_at_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .bind(state.last_get_status.map(|x| x as i64))
    .bind(state.last_get_error.map(|e| format!("{:?}", e)))
    .bind(&state.etag)
    .bind(state.last_modified_ms)
    .bind(state.last_modified_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .bind(state.backoff_index as i64)
    .bind(state.base_poll_seconds as i64)
    .bind(state.next_action_at_ms)
    .bind(epoch_ms_to_iso(state.next_action_at_ms, zone))
    .bind(state.jitter_seconds)
    .execute(&self.pool)
    .await
    .map_err(|e| format!("insert_state current error: {e}"))?;

    Ok(())
  }

  async fn insert_event(
    &self,
    feed_id: &str,
    method: &str,
    status: Option<i64>,
    error_kind: Option<ErrorKind>,
    latency_ms: Option<i64>,
    backoff_index: i64,
    scheduled_next_action_at_ms: i64,
    debug: Option<&str>,
    zone: &Tz,
  ) -> Result<(), String> {
    let now_ms = now_epoch_ms();
    sqlx::query(
      r#"
      INSERT INTO fetch_events(
        feed_id, event_time_ms, event_time_text, method,
        status, error_kind, latency_ms, backoff_index,
        scheduled_next_action_at_ms, scheduled_next_action_at_text, debug
      ) VALUES (
        ?1, ?2, ?3, ?4,
        ?5, ?6, ?7, ?8,
        ?9, ?10, ?11
      )
      "#,
    )
    .bind(feed_id)
    .bind(now_ms)
    .bind(epoch_ms_to_iso(now_ms, zone))
    .bind(method)
    .bind(status)
    .bind(error_kind.map(|e| format!("{:?}", e)))
    .bind(latency_ms)
    .bind(backoff_index)
    .bind(scheduled_next_action_at_ms)
    .bind(epoch_ms_to_iso(scheduled_next_action_at_ms, zone))
    .bind(debug.map(|s| s.to_string()))
    .execute(&self.pool)
    .await
    .map_err(|e| format!("insert_event error: {e}"))?;
    Ok(())
  }

  async fn insert_payload_with_items(
    &self,
    feed_id: &str,
    fetched_at_ms: i64,
    etag: Option<&str>,
    last_modified_ms: Option<i64>,
    content_hash: Option<&str>,
    parsed: &ParsedFeed,
    zone: &Tz,
  ) -> Result<(), String> {
    let mut tx = self.pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;

    let payload_id: i64 = sqlx::query_scalar(
      r#"
      INSERT INTO feed_payloads(
        feed_id, fetched_at_ms, fetched_at_text, etag,
        last_modified_ms, last_modified_text, content_hash,
        title, link, description, language,
        updated_at_ms, updated_at_text
      ) VALUES (
        ?1, ?2, ?3, ?4,
        ?5, ?6, ?7,
        ?8, ?9, ?10, ?11,
        ?12, ?13
      );
      SELECT last_insert_rowid();
      "#,
    )
    .bind(feed_id)
    .bind(fetched_at_ms)
    .bind(epoch_ms_to_iso(fetched_at_ms, zone))
    .bind(etag.map(|s| s.to_string()))
    .bind(last_modified_ms)
    .bind(last_modified_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .bind(content_hash.map(|s| s.to_string()))
    .bind(parsed.metadata.title.clone())
    .bind(parsed.metadata.link.clone())
    .bind(parsed.metadata.description.clone())
    .bind(parsed.metadata.language.clone())
    .bind(parsed.metadata.updated_at_ms)
    .bind(parsed.metadata.updated_at_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("insert payload: {e}"))?;

    for it in &parsed.items {
      sqlx::query(
        r#"
        INSERT INTO feed_items(
          payload_id, feed_id, title, link, guid,
          published_at_ms, published_at_text,
          category, description, summary
        ) VALUES (
          ?1, ?2, ?3, ?4, ?5,
          ?6, ?7,
          ?8, ?9, ?10
        )
        "#,
      )
      .bind(payload_id)
      .bind(feed_id)
      .bind(it.title.clone())
      .bind(it.link.clone())
      .bind(it.guid.clone())
      .bind(it.published_at_ms)
      .bind(it.published_at_ms.map(|ms| epoch_ms_to_iso(ms, zone)))
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
}

fn now_epoch_ms() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as i64
}
