use std::path::Path;

use sqlx::{
  Pool,
  Postgres,
  Sqlite
};
use tokio::fs;

use crate::app_state::AppState;
use crate::config::{
  ConfigError,
  ServerConfig,
  SqlDialect,
  validate_schema_name
};
use crate::db::quote_ident;

pub async fn apply_server_schema(
  config: &ServerConfig,
  state: &AppState,
  config_path: &Path
) -> Result<(), ConfigError> {
  let base_dir = config_path
    .parent()
    .ok_or_else(|| {
    ConfigError::Invalid(
      "config path has no parent"
        .into()
    )
  })?;

  match config.dialect()? {
    | SqlDialect::Sqlite => {
      let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| {
          ConfigError::Invalid(
            "sqlite pool missing"
              .into()
          )
        })?;

      let schema_path = base_dir
        .join("sql")
        .join("sqlite")
        .join("schema.sql");

      let content = fs::read_to_string(
        &schema_path
      )
      .await
      .map_err(|_| {
        ConfigError::Invalid(format!(
          "schema not found at {}",
          schema_path.display()
        ))
      })?;

      execute_schema_sqlite(
        pool, &content
      )
      .await?;
      ensure_admin_role_sqlite(pool)
        .await?;
    }
    | SqlDialect::Postgres => {
      let pool = state
        .postgres
        .as_ref()
        .ok_or_else(|| {
          ConfigError::Invalid(
            "postgres pool missing"
              .into()
          )
        })?;

      let pg = config
        .postgres
        .as_ref()
        .ok_or_else(|| {
          ConfigError::Invalid(
            "postgres section missing"
              .into()
          )
        })?;

      let schema =
        validate_schema_name(
          &pg.schema
        )?;

      let fetcher_schema =
        validate_schema_name(
          &pg.fetcher_schema
        )?;

      let schema_path = base_dir
        .join("sql")
        .join("postgres")
        .join("schema.sql");

      let content = fs::read_to_string(
        &schema_path
      )
      .await
      .map_err(|_| {
        ConfigError::Invalid(format!(
          "schema not found at {}",
          schema_path.display()
        ))
      })?;

      execute_schema_postgres(
        pool,
        &content,
        &schema,
        &fetcher_schema
      )
      .await?;
      sqlx::query(
        "ALTER TABLE users ADD COLUMN \
         IF NOT EXISTS is_admin \
         BOOLEAN NOT NULL DEFAULT \
         FALSE"
      )
      .execute(pool)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "admin role migration \
           failed: {e}"
        ))
      })?;
    }
  }

  Ok(())
}

async fn execute_schema_sqlite(
  pool: &Pool<Sqlite>,
  content: &str
) -> Result<(), ConfigError> {
  for stmt in content.split(';') {
    let trimmed = stmt.trim();

    if trimmed.is_empty() {
      continue;
    }

    sqlx::query(trimmed)
      .execute(pool)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "schema apply error: {e}"
        ))
      })?;
  }

  Ok(())
}

async fn execute_schema_postgres(
  pool: &Pool<Postgres>,
  content: &str,
  schema: &str,
  fetcher_schema: &str
) -> Result<(), ConfigError> {
  let mut conn = pool
    .acquire()
    .await
    .map_err(|e| {
    ConfigError::Invalid(format!(
      "schema apply error: {e}"
    ))
  })?;

  let search_stmt = format!(
    "SET search_path TO {}, {}",
    quote_ident(schema),
    quote_ident(fetcher_schema)
  );

  sqlx::query(&search_stmt)
    .execute(&mut *conn)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "schema apply error: {e}"
      ))
    })?;

  for stmt in content.split(';') {
    let trimmed = stmt.trim();

    if trimmed.is_empty() {
      continue;
    }

    sqlx::query(trimmed)
      .execute(&mut *conn)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "schema apply error: {e}"
        ))
      })?;
  }

  Ok(())
}

async fn ensure_admin_role_sqlite(
  pool: &Pool<Sqlite>
) -> Result<(), ConfigError> {
  let exists =
    sqlx::query_scalar::<_, i64>(
      "SELECT 1 FROM \
       pragma_table_info('users') \
       WHERE name = 'is_admin' LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "admin role lookup failed: {e}"
      ))
    })?;
  if exists.is_none() {
    sqlx::query(
      "ALTER TABLE users ADD COLUMN \
       is_admin INTEGER NOT NULL \
       DEFAULT 0"
    )
    .execute(pool)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "admin role migration failed: \
         {e}"
      ))
    })?;
  }
  Ok(())
}

#[cfg(test)]
mod schema_security_tests {
  use sqlx::sqlite::SqlitePoolOptions;

  use super::*;

  #[tokio::test]
  async fn existing_sqlite_users_are_migrated_without_role_escalation()
   {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("sqlite");
    sqlx::query(
      "CREATE TABLE users (id INTEGER \
       PRIMARY KEY, username TEXT NOT \
       NULL, password_hash TEXT NOT \
       NULL, created_at TEXT NOT NULL)"
    )
    .execute(&pool)
    .await
    .expect("legacy table");
    sqlx::query(
      "INSERT INTO users \
       (id,username,password_hash,\
       created_at) VALUES \
       (9,'legacy','hash','2026-01-01'\
       )"
    )
    .execute(&pool)
    .await
    .expect("legacy user");
    ensure_admin_role_sqlite(&pool)
      .await
      .expect("migrate once");
    ensure_admin_role_sqlite(&pool)
      .await
      .expect("migrate twice");
    let (id, admin): (i64, i64) =
      sqlx::query_as(
        "SELECT id, is_admin FROM \
         users WHERE username='legacy'"
      )
      .fetch_one(&pool)
      .await
      .expect("existing user");
    assert_eq!(id, 9);
    assert_eq!(admin, 0);
  }
}

#[cfg(test)]
mod sqlite_recovery_tests {
  use std::time::{SystemTime, UNIX_EPOCH};

  use axum::Json;
  use axum::extract::{Path as AxumPath, Query, State};
  use axum::http::{HeaderMap, StatusCode};
  use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

  use super::*;
  use crate::auth::hash_password;
  use crate::config::{
    AppConfig, AppMode, AuthConfig, DatabaseConfig, DevConfig, HttpConfig,
    LoggingConfig, SqliteConfig
  };
  use crate::handlers::{auth, entries};
  use crate::models::{EntryListQuery, LoginRequest};

  fn state(pool: sqlx::SqlitePool) -> AppState {
    AppState {
      sqlite: Some(pool),
      postgres: None,
      fetcher_schema: None,
      token_ttl_seconds: 3600,
    }
  }

  fn config() -> ServerConfig {
    ServerConfig {
      app: AppConfig { mode: AppMode::Prod, timezone: None },
      http: HttpConfig { host: "127.0.0.1".into(), port: 8091 },
      database: DatabaseConfig { dialect: "sqlite".into() },
      sqlite: SqliteConfig { path: "unused.sqlite".into() },
      postgres: None,
      logging: LoggingConfig { level: Some("warn".into()) },
      auth: AuthConfig { token_ttl_seconds: 3600 },
      dev: DevConfig { reset_on_start: false },
    }
  }

  async fn connect(path: &Path, create: bool) -> sqlx::SqlitePool {
    SqlitePoolOptions::new()
      .max_connections(1)
      .connect_with(
        SqliteConnectOptions::new()
          .filename(path)
          .create_if_missing(create)
          .foreign_keys(true)
      )
      .await
      .expect("connect isolated SQLite file")
  }

  #[tokio::test]
  async fn legacy_sqlite_schema_backup_restore_preserves_reader_state() {
    let stamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("monotonic epoch")
      .as_nanos();
    let root = std::env::temp_dir().join(format!(
      "pulsewire-sqlite-recovery-{}-{stamp}", std::process::id()
    ));
    let sql_dir = root.join("sql/sqlite");
    std::fs::create_dir_all(&sql_dir).expect("isolated schema directory");
    std::fs::write(
      sql_dir.join("schema.sql"),
      include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/res/sql/sqlite/schema.sql"))
    ).expect("write current server schema fixture");

    let original = root.join("original.sqlite");
    let snapshot = root.join("snapshot.sqlite");
    let restored = root.join("restored.sqlite");
    let config_path = root.join("config.toml");
    let cfg = config();
    let pool = connect(&original, true).await;
    let core_sql = include_str!(concat!(
      env!("CARGO_MANIFEST_DIR"), "/../core/res/sql/sqlite/schema.sql"
    ));
    execute_schema_sqlite(&pool, core_sql).await.expect("fetcher tables");

    // Simulate an existing account database from before the is_admin column.
    sqlx::query(
      "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL, created_at TEXT NOT NULL)"
    )
      .execute(&pool).await.expect("legacy users");
    let password = "sqlite-legacy-passphrase";
    let hash = hash_password(password).expect("legacy password hash");
    sqlx::query(
      "INSERT INTO users (id, username, password_hash, created_at) VALUES (7, 'old_reader', ?1, '2026-01-01')"
    )
      .bind(hash).execute(&pool).await.expect("insert legacy reader");

    let initial = state(pool.clone());
    apply_server_schema(&cfg, &initial, &config_path).await.expect("first migration");
    apply_server_schema(&cfg, &initial, &config_path).await.expect("repeat migration");

    let (id, admin): (i64, i64) = sqlx::query_as(
      "SELECT id, is_admin FROM users WHERE username = 'old_reader'"
    ).fetch_one(&pool).await.expect("migrated old reader");
    assert_eq!(id, 7);
    assert_eq!(admin, 0, "legacy account must never inherit admin privileges");

    sqlx::query("INSERT INTO categories (name, created_at_ms) VALUES ('news', 100)")
      .execute(&pool).await.expect("news category");
    sqlx::query(
      "INSERT INTO feeds (id, url, domain, category, base_poll_seconds, created_at_ms) VALUES ('fixture', 'https://ci.example/rss', 'ci.example', 'news', 3600, 100)"
    ).execute(&pool).await.expect("feed");
    sqlx::query(
      "INSERT INTO feed_payloads (id, feed_id, fetched_at_ms, title) VALUES (3, 'fixture', 100, 'Test Feed')"
    ).execute(&pool).await.expect("payload");
    sqlx::query(
      "INSERT INTO feed_items (id, payload_id, feed_id, title, published_at_ms) VALUES (11, 3, 'fixture', 'Preserved SQLite Article', 100)"
    ).execute(&pool).await.expect("feed item");

    crate::db::bootstrap_admin(&initial, "old_reader", password)
      .await.expect("promote existing account only after credential proof");
    let Json(login) = auth::login(
      State(initial.clone()),
      Json(LoginRequest { username: "old_reader".into(), password: password.into() })
    ).await.expect("log in after migration");
    let mut headers = HeaderMap::new();
    headers.insert(
      "authorization",
      format!("Bearer {}", login.token).parse().expect("bearer token")
    );
    assert_eq!(
      entries::mark_read(
        State(initial.clone()), headers.clone(), AxumPath(11)
      ).await.expect("mark fixture read"),
      StatusCode::NO_CONTENT
    );

    let Json(before) = entries::list_entries(
      State(initial.clone()), headers.clone(), Query(EntryListQuery {
        read: Some("read".into()), limit: None, offset: None,
        feed_id: Some("fixture".into()), since: None
      })
    ).await.expect("list original reader state");
    assert_eq!(before.items.len(), 1);
    assert_eq!(before.items[0].title.as_deref(), Some("Preserved SQLite Article"));
    assert!(before.items[0].is_read);

    // VACUUM INTO creates a consistent online snapshot, not an unsafe live-file copy.
    sqlx::query("VACUUM INTO ?1")
      .bind(snapshot.to_string_lossy().to_string())
      .execute(&pool).await.expect("SQLite consistent backup");
    assert!(snapshot.metadata().expect("snapshot exists").len() > 0);
    initial.sqlite.as_ref().expect("pool").close().await;
    drop(initial);
    drop(pool);

    std::fs::copy(&snapshot, &restored).expect("restore to a distinct file");
    let recovered_pool = connect(&restored, false).await;
    let recovered = state(recovered_pool.clone());
    apply_server_schema(&cfg, &recovered, &config_path).await.expect("restored migration replay");
    apply_server_schema(&cfg, &recovered, &config_path).await.expect("idempotent replay");

    let Json(after) = entries::list_entries(
      State(recovered.clone()), headers.clone(), Query(EntryListQuery {
        read: Some("read".into()), limit: None, offset: None,
        feed_id: Some("fixture".into()), since: None
      })
    ).await.expect("list restored data through reader API");
    assert_eq!(after.items.len(), 1);
    assert_eq!(after.items[0].id, 11);
    assert_eq!(after.items[0].title.as_deref(), Some("Preserved SQLite Article"));
    assert!(after.items[0].is_read);
    assert_eq!(
      entries::read_state(State(recovered.clone()), headers, AxumPath(11))
        .await.expect("read-state survives restore"),
      StatusCode::NO_CONTENT
    );

    let admin_after: i64 = sqlx::query_scalar(
      "SELECT is_admin FROM users WHERE id = 7"
    ).fetch_one(&recovered_pool).await.expect("role survives restore");
    assert_eq!(admin_after, 1);
    recovered_pool.close().await;
    std::fs::remove_dir_all(root).expect("remove isolated database fixtures");
  }
}
