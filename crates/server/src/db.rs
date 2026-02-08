use std::path::Path;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::app_state::AppState;
use crate::auth::hash_password;
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

pub async fn reset_server_data(
  config: &ServerConfig,
  state: &AppState
) -> Result<(), ConfigError> {
  let tables = [
    "user_tokens",
    "user_password_resets",
    "favorites",
    "entry_states",
    "folder_feeds",
    "folders",
    "subscriptions",
    "users"
  ];

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

      for table in tables {
        let query = format!(
          "DELETE FROM {table}"
        );

        if let Err(e) =
          sqlx::query(&query)
            .execute(pool)
            .await
          && !is_missing_table_error(&e)
        {
          return Err(
            ConfigError::Invalid(
              format!(
                "cleanup {table} \
                 failed: {e}"
              )
            )
          );
        }
      }
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

      let schema = config
        .postgres
        .as_ref()
        .ok_or_else(|| {
          ConfigError::Invalid(
            "postgres section missing"
              .into()
          )
        })?
        .schema
        .as_str();

      let schema =
        validate_schema_name(schema)?;

      let table_list = tables
        .iter()
        .map(|t| {
          format!(
            "{}.{}",
            quote_ident(&schema),
            quote_ident(t)
          )
        })
        .collect::<Vec<_>>()
        .join(", ");

      let stmt = format!(
        "TRUNCATE TABLE {table_list} \
         RESTART IDENTITY CASCADE"
      );

      if let Err(e) = sqlx::query(&stmt)
        .execute(pool)
        .await
        && !is_missing_table_error(&e)
      {
        return Err(
          ConfigError::Invalid(
            format!(
              "cleanup failed: {e}"
            )
          )
        );
      }
    }
  }

  Ok(())
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

pub async fn ensure_default_user(
  config: &ServerConfig,
  state: &AppState,
  username: &str,
  password: &str
) -> Result<(), ConfigError> {
  let password_hash = hash_password(
    password
  )
  .map_err(|e| {
    ConfigError::Invalid(format!(
      "hash password: {e}"
    ))
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

      let result = sqlx::query(
        "INSERT OR IGNORE INTO users \
         (username, password_hash, \
         created_at) VALUES (?1, ?2, \
         datetime('now'))"
      )
      .bind(username)
      .bind(password_hash)
      .execute(pool)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "default user insert \
           failed: {e}"
        ))
      })?;

      if result.rows_affected() > 0 {
        tracing::info!(
          username,
          "default user created"
        );
      }
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

      let result = sqlx::query(
        "INSERT INTO users (username, \
         password_hash, created_at) \
         VALUES ($1, $2, NOW()) ON \
         CONFLICT (username) DO \
         NOTHING"
      )
      .bind(username)
      .bind(password_hash)
      .execute(pool)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "default user insert \
           failed: {e}"
        ))
      })?;

      if result.rows_affected() > 0 {
        tracing::info!(
          username,
          "default user created"
        );
      }
    }
  }

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
