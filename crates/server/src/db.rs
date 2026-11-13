use std::path::Path;

use sqlx::PgPool;
use sqlx::postgres::{
  PgConnectOptions,
  PgPoolOptions,
  PgSslMode
};

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

      // Build options from structured
      // values. Do not interpolate
      // credentials into a URL:
      // reserved characters are valid
      // in passwords.
      let options =
        postgres_connect_options(pg)?;

      let pool = PgPoolOptions::new()
        .max_connections(10)
        .after_connect(set_search_path(
          schema.clone()
        ))
        .connect_with(options)
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

/// Construct PostgreSQL options without
/// interpreting credentials as a URL.
fn postgres_connect_options(
  pg: &crate::config::PostgresConfig
) -> Result<PgConnectOptions, ConfigError>
{
  let ssl_mode = match pg
    .ssl_mode
    .trim()
    .to_ascii_lowercase()
    .as_str()
  {
    | "disable" => PgSslMode::Disable,
    | "prefer" => PgSslMode::Prefer,
    | "require" => PgSslMode::Require,
    | "verify-ca" => {
      PgSslMode::VerifyCa
    }
    | "verify-full" => {
      PgSslMode::VerifyFull
    }
    | other => {
      return Err(ConfigError::Invalid(
        format!(
          "invalid postgres.ssl_mode \
           '{other}': expected \
           disable, prefer, require, \
           verify-ca or verify-full"
        )
      ))
    }
  };

  Ok(
    PgConnectOptions::new()
      .host(&pg.host)
      .port(pg.port)
      .username(&pg.user)
      .password(&pg.password)
      .database(&pg.database)
      .ssl_mode(ssl_mode)
  )
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
  if username.trim().is_empty()
    || password.len() < 12
  {
    return Err(ConfigError::Invalid(
      "bootstrap requires username \
       and password at least 12 bytes \
       long"
        .into()
    ));
  }
  if let Some(pool) = &state.postgres {
    let mut tx = pool
      .begin()
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "bootstrap begin: {e}"
        ))
      })?;
    let count: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM users"
      )
      .fetch_one(&mut *tx)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "bootstrap count: {e}"
        ))
      })?;
    if count == 0 {
      let hash =
        hash_password(password)
          .map_err(
            ConfigError::Invalid
          )?;
      sqlx::query(
        "INSERT INTO users (username, \
         password_hash, is_admin, \
         created_at) VALUES ($1, $2, \
         TRUE, NOW())"
      )
      .bind(username)
      .bind(hash)
      .execute(&mut *tx)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "bootstrap insert: {e}"
        ))
      })?;
    } else {
      let row = sqlx::query_as::<
        _,
        (i64, String, bool)
      >(
        "SELECT id, password_hash, \
         is_admin FROM users WHERE \
         username = $1"
      )
      .bind(username)
      .fetch_optional(&mut *tx)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "bootstrap lookup: {e}"
        ))
      })?;
      let Some((id, hash, admin)) = row
      else {
        return Err(
          ConfigError::Invalid(
            "database already \
             contains users; select \
             an existing account for \
             bootstrap"
              .into()
          )
        );
      };
      if admin {
        return Err(
          ConfigError::Invalid(
            "account is already an \
             administrator"
              .into()
          )
        );
      }
      verify_password(&hash, password)
        .map_err(|_| {
          ConfigError::Invalid(
            "existing account \
             credentials do not match"
              .into()
          )
        })?;
      sqlx::query(
        "UPDATE users SET is_admin = \
         TRUE WHERE id = $1"
      )
      .bind(id)
      .execute(&mut *tx)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "bootstrap promote: {e}"
        ))
      })?;
      sqlx::query(
        "DELETE FROM user_tokens \
         WHERE user_id = $1"
      )
      .bind(id)
      .execute(&mut *tx)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "bootstrap revoke: {e}"
        ))
      })?;
    }
    tx.commit().await.map_err(|e| {
      ConfigError::Invalid(format!(
        "bootstrap commit: {e}"
      ))
    })?;
    return Ok(());
  }
  let pool = state
    .sqlite
    .as_ref()
    .ok_or_else(|| {
      ConfigError::Invalid(
        "sqlite pool missing".into()
      )
    })?;
  let mut tx = pool
    .begin()
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "bootstrap begin: {e}"
      ))
    })?;
  let count: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM users"
  )
  .fetch_one(&mut *tx)
  .await
  .map_err(|e| {
    ConfigError::Invalid(format!(
      "bootstrap count: {e}"
    ))
  })?;
  if count == 0 {
    let hash = hash_password(password)
      .map_err(ConfigError::Invalid)?;
    sqlx::query(
      "INSERT INTO users (username, \
       password_hash, is_admin, \
       created_at) VALUES (?1, ?2, 1, \
       datetime('now'))"
    )
    .bind(username)
    .bind(hash)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "bootstrap insert: {e}"
      ))
    })?;
  } else {
    let row = sqlx::query_as::<
      _,
      (i64, String, i64)
    >(
      "SELECT id, password_hash, \
       is_admin FROM users WHERE \
       username = ?1"
    )
    .bind(username)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "bootstrap lookup: {e}"
      ))
    })?;
    let Some((id, hash, admin)) = row
    else {
      return Err(ConfigError::Invalid(
        "database already contains \
         users; select an existing \
         account for bootstrap"
          .into()
      ));
    };
    if admin != 0 {
      return Err(ConfigError::Invalid(
        "account is already an \
         administrator"
          .into()
      ));
    }
    verify_password(&hash, password)
      .map_err(|_| {
        ConfigError::Invalid(
          "existing account \
           credentials do not match"
            .into()
        )
      })?;
    sqlx::query(
      "UPDATE users SET is_admin = 1 \
       WHERE id = ?1"
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "bootstrap promote: {e}"
      ))
    })?;
    sqlx::query(
      "DELETE FROM user_tokens WHERE \
       user_id = ?1"
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "bootstrap revoke: {e}"
      ))
    })?;
  }
  tx.commit().await.map_err(|e| {
    ConfigError::Invalid(format!(
      "bootstrap commit: {e}"
    ))
  })?;
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
  use sqlx::sqlite::SqlitePoolOptions;

  use super::*;

  #[tokio::test]
  async fn existing_user_requires_correct_password_for_admin_promotion()
   {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("sqlite");
    sqlx::query(
      "CREATE TABLE users (id INTEGER \
       PRIMARY KEY, username TEXT NOT \
       NULL UNIQUE, password_hash \
       TEXT NOT NULL, is_admin \
       INTEGER NOT NULL DEFAULT 0, \
       created_at TEXT NOT NULL)"
    )
    .execute(&pool)
    .await
    .expect("users");
    sqlx::query(
      "CREATE TABLE user_tokens (id \
       INTEGER PRIMARY KEY, user_id \
       INTEGER NOT NULL, token_hash \
       TEXT NOT NULL, expires_at TEXT \
       NOT NULL, created_at TEXT NOT \
       NULL)"
    )
    .execute(&pool)
    .await
    .expect("tokens");
    let hash = hash_password(
      "original-long-password"
    )
    .expect("hash");
    sqlx::query(
      "INSERT INTO users \
       (id,username,password_hash,\
       created_at) VALUES \
       (1,'legacy',?1,'2026-01-01')"
    )
    .bind(hash)
    .execute(&pool)
    .await
    .expect("legacy account");
    sqlx::query(
      "INSERT INTO user_tokens \
       (user_id,token_hash,expires_at,\
       created_at) VALUES \
       (1,'token','2099-01-01','\
       2026-01-01')"
    )
    .execute(&pool)
    .await
    .expect("existing token");
    let state = AppState {
      sqlite:            Some(
        pool.clone()
      ),
      postgres:          None,
      fetcher_schema:    None,
      token_ttl_seconds: 3600
    };
    assert!(
      bootstrap_admin(
        &state,
        "legacy",
        "wrong-password-long"
      )
      .await
      .is_err()
    );
    let before: i64 =
      sqlx::query_scalar(
        "SELECT is_admin FROM users \
         WHERE id=1"
      )
      .fetch_one(&pool)
      .await
      .expect("before role");
    assert_eq!(before, 0);
    bootstrap_admin(
      &state,
      "legacy",
      "original-long-password"
    )
    .await
    .expect("promote with proof");
    let after: i64 =
      sqlx::query_scalar(
        "SELECT is_admin FROM users \
         WHERE id=1"
      )
      .fetch_one(&pool)
      .await
      .expect("after role");
    assert_eq!(after, 1);
    let tokens: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM \
         user_tokens"
      )
      .fetch_one(&pool)
      .await
      .expect("tokens");
    assert_eq!(tokens, 0);
  }
}

#[cfg(test)]
mod postgres_runtime_tests {
  use axum::Json;
  use axum::extract::State;
  use axum::http::HeaderMap;

  use super::*;
  use crate::config::{
    AppConfig,
    AppMode,
    AuthConfig,
    DatabaseConfig,
    DevConfig,
    HttpConfig,
    LoggingConfig,
    PostgresConfig,
    SqliteConfig
  };
  use crate::handlers::{
    auth,
    users
  };
  use crate::models::{
    CreateUserRequest,
    LoginRequest,
    PasswordChangeRequest
  };

  fn postgres_fixture(
    password: String
  ) -> PostgresConfig {
    PostgresConfig {
      host: "127.0.0.1".into(),
      port: 5432,
      database: "pulsewire_ci".into(),
      user: "pulsewire_ci".into(),
      password,
      ssl_mode: "disable".into(),
      schema: "pulsewire_ci_server"
        .into(),
      fetcher_schema:
        "pulsewire_ci_fetcher".into()
    }
  }

  #[test]
  fn rejects_unknown_postgres_tls_mode_without_leaking_credentials()
   {
    let mut config = postgres_fixture(
      "secret@with:/?#".into()
    );
    config.ssl_mode =
      "not-a-mode".into();
    let error =
      postgres_connect_options(&config)
        .expect_err(
          "invalid TLS option"
        );
    let printed = error.to_string();
    assert!(printed.contains(
      "invalid postgres.ssl_mode"
    ));
    assert!(
      !printed
        .contains("secret@with:/?#")
    );
  }

  /// Runs in a disposable GitHub
  /// Actions PostgreSQL service, never
  /// against a workstation's real
  /// database. It is ignored in the
  /// default unit suite.
  #[tokio::test]
  #[ignore = "requires isolated CI \
              PostgreSQL service"]
  async fn postgres_runtime_schemas_auth_and_password_rotation()
   {
    let password = std::env::var("PULSEWIRE_TEST_POSTGRES_PASSWORD")
      .expect("isolated PostgreSQL test password required");
    assert!(
      password.contains('@')
        && password.contains('/')
        && password.contains('#')
    );

    let pg = postgres_fixture(password);
    let pool = PgPoolOptions::new()
      .max_connections(2)
      .connect_with(
        postgres_connect_options(&pg)
          .expect("connection options")
      )
      .await
      .expect(
        "punctuation-heavy password \
         must connect"
      );

    // This job's PostgreSQL database is
    // disposable. No destructive DROP.
    sqlx::query(
      "CREATE SCHEMA IF NOT EXISTS \
       pulsewire_ci_fetcher"
    )
    .execute(&pool)
    .await
    .expect("fetcher schema");
    sqlx::query(
      "CREATE SCHEMA IF NOT EXISTS \
       pulsewire_ci_server"
    )
    .execute(&pool)
    .await
    .expect("server schema");
    sqlx::query(
      "CREATE TABLE IF NOT EXISTS \
       pulsewire_ci_fetcher.feeds (id \
       TEXT PRIMARY KEY)"
    )
    .execute(&pool)
    .await
    .expect("feeds");
    sqlx::query(
      "CREATE TABLE IF NOT EXISTS \
       pulsewire_ci_fetcher.\
       feed_items (id BIGINT PRIMARY \
       KEY)"
    )
    .execute(&pool)
    .await
    .expect("feed_items");

    // Simulate an existing database
    // from before the is_admin
    // migration.
    sqlx::query(
      "CREATE TABLE IF NOT EXISTS \
       pulsewire_ci_server.users (id \
       BIGSERIAL PRIMARY KEY, \
       username TEXT NOT NULL UNIQUE, \
       password_hash TEXT NOT NULL, \
       created_at TIMESTAMPTZ NOT \
       NULL)"
    )
    .execute(&pool)
    .await
    .expect("legacy users");
    let legacy_password =
      "existing-legacy-passphrase";
    let hash =
      hash_password(legacy_password)
        .expect("hash");
    sqlx::query(
      "INSERT INTO \
       pulsewire_ci_server.users \
       (username, password_hash, \
       created_at) VALUES ($1, $2, \
       NOW())"
    )
    .bind("existing_user")
    .bind(hash)
    .execute(&pool)
    .await
    .expect("insert legacy user");
    drop(pool);

    let config = ServerConfig {
      app:      AppConfig {
        mode:     AppMode::Prod,
        timezone: None
      },
      http:     HttpConfig {
        host: "127.0.0.1".into(),
        port: 8091
      },
      database: DatabaseConfig {
        dialect: "postgres".into()
      },
      sqlite:   SqliteConfig {
        path: "unused.sqlite".into()
      },
      postgres: Some(pg),
      logging:  LoggingConfig {
        level: Some("warn".into())
      },
      auth:     AuthConfig {
        token_ttl_seconds: 3600
      },
      dev:      DevConfig {
        reset_on_start: false
      }
    };
    let config_path =
      Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/res/config.toml"
      ));
    let state =
      connect_db(&config, config_path)
        .await
        .expect("server connection");
    crate::schema::apply_server_schema(
      &config,
      &state,
      config_path
    )
    .await
    .expect("apply server migrations");
    crate::schema::apply_server_schema(
      &config,
      &state,
      config_path
    )
    .await
    .expect(
      "migrations must be repeatable"
    );

    let admin_before: bool =
      sqlx::query_scalar(
        "SELECT is_admin FROM users \
         WHERE username = \
         'existing_user'"
      )
      .fetch_one(
        state
          .postgres
          .as_ref()
          .expect("postgres pool")
      )
      .await
      .expect("role migrated");
    assert!(
      !admin_before,
      "old accounts must not become \
       admins automatically"
    );

    assert!(
      bootstrap_admin(
        &state,
        "existing_user",
        "wrong-passphrase"
      )
      .await
      .is_err()
    );
    bootstrap_admin(
      &state,
      "existing_user",
      legacy_password
    )
    .await
    .expect(
      "promote only with existing \
       credential"
    );

    let Json(token) = auth::login(
      State(state.clone()),
      Json(LoginRequest {
        username: "existing_user"
          .into(),
        password: legacy_password
          .into()
      })
    )
    .await
    .expect("login");
    let mut headers = HeaderMap::new();
    headers.insert(
      "authorization",
      format!("Bearer {}", token.token)
        .parse()
        .expect("token header")
    );
    let Json(reader) =
      users::create_user(
        State(state.clone()),
        headers.clone(),
        Json(CreateUserRequest {
          username: "new_reader".into(),
          password:
            "reader-passphrase-long"
              .into()
        })
      )
      .await
      .expect("admin creates reader");
    assert_eq!(
      reader.username,
      "new_reader"
    );
    users::change_password(
      State(state.clone()),
      headers.clone(),
      Json(PasswordChangeRequest {
        current_password:
          legacy_password.into(),
        new_password:
          "updated-legacy-passphrase"
            .into()
      })
    )
    .await
    .expect("change password");
    assert!(
      crate::auth::auth_user_id(
        &state, &headers
      )
      .await
      .is_err()
    );
    let Json(fresh) = auth::login(
      State(state.clone()),
      Json(LoginRequest {
        username: "existing_user"
          .into(),
        password:
          "updated-legacy-passphrase"
            .into()
      })
    )
    .await
    .expect("new password works");
    assert!(!fresh.token.is_empty());
  }
}
