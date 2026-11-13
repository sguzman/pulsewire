use std::path::Path;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::app_state::AppState;
use crate::auth::{
  hash_password,
  verify_password
};
use crate::config::{
  ConfigError,
  ServerConfig,
  SqlDialect,
  validate_schema_name
};

pub async fn connect_db(
  config: &ServerConfig,
  config_path: &Path
) -> Result<AppState, ConfigError> {
  match config.dialect()? {
    | SqlDialect::Sqlite => {
      let base_dir = config_path
        .parent()
        .ok_or_else(|| {
          ConfigError::Invalid(
            "config path has no parent"
              .into()
          )
        })?;

      let path =
        config.sqlite_path(base_dir);

      let url = format!(
        "sqlite://{}",
        path.display()
      );

      let pool =
        sqlx::SqlitePool::connect(&url)
          .await
          .map_err(|e| {
            ConfigError::Invalid(
              format!(
                "sqlite connect \
                 failed: {e}"
              )
            )
          })?;

      ensure_fetcher_tags_column_sqlite(&pool).await?;

      Ok(AppState {
        sqlite:            Some(pool),
        postgres:          None,
        fetcher_schema:    None,
        token_ttl_seconds: config
          .auth
          .token_ttl_seconds
      })
    }
    | SqlDialect::Postgres => {
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

      let url = format!(
        "postgres://{}:{}@{}:{}/{}?\
         sslmode={}",
        pg.user,
        pg.password,
        pg.host,
        pg.port,
        pg.database,
        pg.ssl_mode
      );

      let pool = PgPoolOptions::new()
        .max_connections(10)
        .after_connect(set_search_path(
          schema.clone()
        ))
        .connect(&url)
        .await
        .map_err(|e| {
          ConfigError::Invalid(format!(
            "postgres connect failed: \
             {e}"
          ))
        })?;

      ensure_fetcher_tags_column_postgres(
        &pool,
        &fetcher_schema
      )
      .await?;

      Ok(AppState {
        sqlite:            None,
        postgres:          Some(pool),
        fetcher_schema:    Some(
          fetcher_schema
        ),
        token_ttl_seconds: config
          .auth
          .token_ttl_seconds
      })
    }
  }
}

#[allow(clippy::type_complexity)]
pub fn set_search_path(
  schema: String
) -> impl Fn(
  &mut sqlx::PgConnection,
  sqlx::pool::PoolConnectionMetadata
) -> std::pin::Pin<
  Box<
    dyn std::future::Future<
        Output = Result<
          (),
          sqlx::Error
        >
      > + Send
      + '_
  >
> {
  let schema_name = schema;

  move |conn, _meta| {
    let schema_copy =
      schema_name.clone();

    Box::pin(async move {
      let schema_ident =
        quote_ident(&schema_copy);

      let create_stmt = format!(
        "CREATE SCHEMA IF NOT EXISTS \
         {schema_ident}"
      );

      sqlx::query(&create_stmt)
        .execute(&mut *conn)
        .await?;

      let search_stmt = format!(
        "SET search_path TO \
         {schema_ident}"
      );

      sqlx::query(&search_stmt)
        .execute(&mut *conn)
        .await?;

      Ok(())
    })
  }
}

fn is_missing_table_error(
  e: &sqlx::Error
) -> bool {
  matches!(
      e,
      sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("42P01")
  )
}

pub fn quote_ident(
  name: &str
) -> String {
  format!(
    "\"{}\"",
    name.replace('"', "\"\"")
  )
}

pub async fn bootstrap_admin(
  state: &AppState,
  username: &str,
  password: &str
) -> Result<(), ConfigError> {
  if username.trim().is_empty() || password.len() < 12 {
    return Err(ConfigError::Invalid(
      "bootstrap requires username and password at least 12 bytes long".into()
    ));
  }
  if let Some(pool) = &state.postgres {
    let mut tx = pool.begin().await
      .map_err(|e| ConfigError::Invalid(format!("bootstrap begin: {e}")))?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
      .fetch_one(&mut *tx).await
      .map_err(|e| ConfigError::Invalid(format!("bootstrap count: {e}")))?;
    if count == 0 {
      let hash = hash_password(password).map_err(ConfigError::Invalid)?;
      sqlx::query("INSERT INTO users (username, password_hash, is_admin, created_at) VALUES ($1, $2, TRUE, NOW())")
        .bind(username).bind(hash).execute(&mut *tx).await
        .map_err(|e| ConfigError::Invalid(format!("bootstrap insert: {e}")))?;
    } else {
      let row = sqlx::query_as::<_, (i64, String, bool)>(
        "SELECT id, password_hash, is_admin FROM users WHERE username = $1"
      ).bind(username).fetch_optional(&mut *tx).await
        .map_err(|e| ConfigError::Invalid(format!("bootstrap lookup: {e}")))?;
      let Some((id, hash, admin)) = row else {
        return Err(ConfigError::Invalid(
          "database already contains users; select an existing account for bootstrap".into()
        ));
      };
      if admin { return Err(ConfigError::Invalid("account is already an administrator".into())); }
      verify_password(&hash, password).map_err(|_| ConfigError::Invalid(
        "existing account credentials do not match".into()
      ))?;
      sqlx::query("UPDATE users SET is_admin = TRUE WHERE id = $1")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| ConfigError::Invalid(format!("bootstrap promote: {e}")))?;
      sqlx::query("DELETE FROM user_tokens WHERE user_id = $1")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| ConfigError::Invalid(format!("bootstrap revoke: {e}")))?;
    }
    tx.commit().await
      .map_err(|e| ConfigError::Invalid(format!("bootstrap commit: {e}")))?;
    return Ok(());
  }
  let pool = state.sqlite.as_ref().ok_or_else(|| {
    ConfigError::Invalid("sqlite pool missing".into())
  })?;
  let mut tx = pool.begin().await
    .map_err(|e| ConfigError::Invalid(format!("bootstrap begin: {e}")))?;
  let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
    .fetch_one(&mut *tx).await
    .map_err(|e| ConfigError::Invalid(format!("bootstrap count: {e}")))?;
  if count == 0 {
    let hash = hash_password(password).map_err(ConfigError::Invalid)?;
    sqlx::query("INSERT INTO users (username, password_hash, is_admin, created_at) VALUES (?1, ?2, 1, datetime('now'))")
      .bind(username).bind(hash).execute(&mut *tx).await
      .map_err(|e| ConfigError::Invalid(format!("bootstrap insert: {e}")))?;
  } else {
    let row = sqlx::query_as::<_, (i64, String, i64)>(
      "SELECT id, password_hash, is_admin FROM users WHERE username = ?1"
    ).bind(username).fetch_optional(&mut *tx).await
      .map_err(|e| ConfigError::Invalid(format!("bootstrap lookup: {e}")))?;
    let Some((id, hash, admin)) = row else {
      return Err(ConfigError::Invalid(
        "database already contains users; select an existing account for bootstrap".into()
      ));
    };
    if admin != 0 { return Err(ConfigError::Invalid("account is already an administrator".into())); }
    verify_password(&hash, password).map_err(|_| ConfigError::Invalid(
      "existing account credentials do not match".into()
    ))?;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?1")
      .bind(id).execute(&mut *tx).await
      .map_err(|e| ConfigError::Invalid(format!("bootstrap promote: {e}")))?;
    sqlx::query("DELETE FROM user_tokens WHERE user_id = ?1")
      .bind(id).execute(&mut *tx).await
      .map_err(|e| ConfigError::Invalid(format!("bootstrap revoke: {e}")))?;
  }
  tx.commit().await
    .map_err(|e| ConfigError::Invalid(format!("bootstrap commit: {e}")))?;
  Ok(())
}

async fn ensure_fetcher_tags_column_postgres(
  pool: &PgPool,
  schema: &str
) -> Result<(), ConfigError> {
  let has_column: Option<i32> =
    sqlx::query_scalar(
      "SELECT 1 FROM \
       information_schema.columns \
       WHERE table_schema = $1 AND \
       table_name = 'feeds' AND \
       column_name = 'tags'"
    )
    .bind(schema)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "fetcher tags column check \
         failed: {e}"
      ))
    })?;

  if has_column.is_some() {
    return Ok(());
  }

  let ddl = format!(
    "ALTER TABLE {}.feeds ADD COLUMN \
     IF NOT EXISTS tags TEXT[]",
    quote_ident(schema)
  );

  sqlx::query(&ddl)
    .execute(pool)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "fetcher tags column add \
         failed: {e}"
      ))
    })?;

  Ok(())
}

async fn ensure_fetcher_tags_column_sqlite(
  pool: &sqlx::SqlitePool
) -> Result<(), ConfigError> {
  let has_table: Option<i32> = sqlx::query_scalar(
        r#"SELECT 1 FROM sqlite_master WHERE type='table' AND name='feeds' LIMIT 1"#
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "fetcher tags table check failed: {e}"
      ))
    })?;

  if has_table.is_none() {
    return Ok(());
  }

  let has_column: Option<i32> = sqlx::query_scalar(
        r#"SELECT 1 FROM pragma_table_info('feeds') WHERE name = 'tags' LIMIT 1"#
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "fetcher tags column check failed: {e}"
      ))
    })?;

  if has_column.is_some() {
    return Ok(());
  }

  sqlx::query(
    "ALTER TABLE feeds ADD COLUMN \
     tags TEXT NULL"
  )
  .execute(pool)
  .await
  .map_err(|e| {
    ConfigError::Invalid(format!(
      "fetcher tags column add \
       failed: {e}"
    ))
  })?;

  Ok(())
}

#[cfg(test)]
mod bootstrap_tests {
  use super::*;
  use sqlx::sqlite::SqlitePoolOptions;

  #[tokio::test]
  async fn existing_user_requires_correct_password_for_admin_promotion() {
    let pool = SqlitePoolOptions::new().max_connections(1)
      .connect("sqlite::memory:").await.expect("sqlite");
    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL, is_admin INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL)")
      .execute(&pool).await.expect("users");
    sqlx::query("CREATE TABLE user_tokens (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, token_hash TEXT NOT NULL, expires_at TEXT NOT NULL, created_at TEXT NOT NULL)")
      .execute(&pool).await.expect("tokens");
    let hash = hash_password("original-long-password").expect("hash");
    sqlx::query("INSERT INTO users (id,username,password_hash,created_at) VALUES (1,'legacy',?1,'2026-01-01')")
      .bind(hash).execute(&pool).await.expect("legacy account");
    sqlx::query("INSERT INTO user_tokens (user_id,token_hash,expires_at,created_at) VALUES (1,'token','2099-01-01','2026-01-01')")
      .execute(&pool).await.expect("existing token");
    let state = AppState {sqlite: Some(pool.clone()), postgres: None, fetcher_schema: None, token_ttl_seconds: 3600};
    assert!(bootstrap_admin(&state, "legacy", "wrong-password-long").await.is_err());
    let before: i64 = sqlx::query_scalar("SELECT is_admin FROM users WHERE id=1")
      .fetch_one(&pool).await.expect("before role");
    assert_eq!(before, 0);
    bootstrap_admin(&state, "legacy", "original-long-password")
      .await.expect("promote with proof");
    let after: i64 = sqlx::query_scalar("SELECT is_admin FROM users WHERE id=1")
      .fetch_one(&pool).await.expect("after role");
    assert_eq!(after, 1);
    let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tokens")
      .fetch_one(&pool).await.expect("tokens");
    assert_eq!(tokens, 0);
  }
}
