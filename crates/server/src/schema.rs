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
      ensure_admin_role_sqlite(pool).await?;
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
      sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN NOT NULL DEFAULT FALSE")
        .execute(pool).await
        .map_err(|e| ConfigError::Invalid(format!("admin role migration failed: {e}")))?;
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
  let exists = sqlx::query_scalar::<_, i64>(
    "SELECT 1 FROM pragma_table_info('users') WHERE name = 'is_admin' LIMIT 1"
  )
    .fetch_optional(pool).await
    .map_err(|e| ConfigError::Invalid(format!("admin role lookup failed: {e}")))?;
  if exists.is_none() {
    sqlx::query("ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0")
      .execute(pool).await
      .map_err(|e| ConfigError::Invalid(format!("admin role migration failed: {e}")))?;
  }
  Ok(())
}
