//! Account-row serialization for bearer
//! token issuance. A password change or
//! offline recovery updates the same
//! user row before revoking sessions.
//! Login and rotation lock that row
//! before writing any new session.

use axum::http::StatusCode;

use crate::app_state::AppState;
use crate::auth::{
  generate_token,
  hash_token
};
use crate::errors::ServerError;

fn database_error(
  err: sqlx::Error
) -> ServerError {
  ServerError::new(
    StatusCode::INTERNAL_SERVER_ERROR,
    err.to_string()
  )
}

fn invalid_credentials() -> ServerError
{
  ServerError::new(
    StatusCode::UNAUTHORIZED,
    "invalid credentials"
  )
}

fn invalid_token() -> ServerError {
  ServerError::new(
    StatusCode::UNAUTHORIZED,
    "invalid token"
  )
}

/// Called only after a password was
/// verified against this exact hash.
/// Re-read that hash under the same
/// account lock as password recovery
/// before issuing a bearer token.
pub(crate) async fn issue_for_verified_login(
  state: &AppState,
  username: &str,
  verified_hash: &str
) -> Result<String, ServerError> {
  let token = generate_token();
  let token_hash = hash_token(&token);
  let ttl =
    state.token_ttl_seconds as i64;

  if let Some(pool) = &state.postgres {
    let mut tx = pool
      .begin()
      .await
      .map_err(database_error)?;
    let current: Option<(i64, String)> =
      sqlx::query_as(
        "SELECT id, password_hash \
         FROM users WHERE username = \
         $1 FOR UPDATE"
      )
      .bind(username)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    let (user_id, current_hash) =
      current.ok_or_else(
        invalid_credentials
      )?;
    if current_hash != verified_hash {
      return Err(invalid_credentials());
    }
    sqlx::query(
      "INSERT INTO user_tokens \
       (user_id, token_hash, \
       expires_at, created_at) VALUES \
       ($1, $2, NOW() + ($3 || ' \
       seconds')::interval, NOW())"
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit()
      .await
      .map_err(database_error)?;
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database pool missing"
      )
    })?;
    let mut tx = pool
      .begin()
      .await
      .map_err(database_error)?;
    // A SQLite UPDATE acquires the
    // write lock before the hash is
    // read. A deferred SELECT would
    // allow a stale snapshot/race with
    // recovery.
    let current: Option<(i64, String)> =
      sqlx::query_as(
        "UPDATE users SET \
         password_hash = \
         password_hash WHERE username \
         = ?1 RETURNING id, \
         password_hash"
      )
      .bind(username)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    let (user_id, current_hash) =
      current.ok_or_else(
        invalid_credentials
      )?;
    if current_hash != verified_hash {
      return Err(invalid_credentials());
    }
    sqlx::query(
      "INSERT INTO user_tokens \
       (user_id, token_hash, \
       expires_at, created_at) VALUES \
       (?1, ?2, datetime('now', '+' \
       || ?3 || ' seconds'), \
       datetime('now'))"
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit()
      .await
      .map_err(database_error)?;
  }

  Ok(token)
}

/// Consumes the original bearer and
/// creates a replacement atomically.
/// The preliminary lookup is advisory:
/// the old token is checked again
/// *after* acquiring the user lock,
/// which also serializes it with
/// account recovery.
pub(crate) async fn rotate_existing_token(
  state: &AppState,
  old_hash: &str
) -> Result<String, ServerError> {
  let token = generate_token();
  let new_hash = hash_token(&token);
  let ttl =
    state.token_ttl_seconds as i64;

  if let Some(pool) = &state.postgres {
    let user_id: i64 =
      sqlx::query_scalar(
        "SELECT user_id FROM \
         user_tokens WHERE token_hash \
         = $1 AND expires_at > NOW()"
      )
      .bind(old_hash)
      .fetch_optional(pool)
      .await
      .map_err(database_error)?
      .ok_or_else(invalid_token)?;
    let mut tx = pool
      .begin()
      .await
      .map_err(database_error)?;
    let locked: Option<i64> =
      sqlx::query_scalar(
        "SELECT id FROM users WHERE \
         id = $1 FOR UPDATE"
      )
      .bind(user_id)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    if locked.is_none() {
      return Err(invalid_token());
    }
    let consumed: Option<i64> =
      sqlx::query_scalar(
        "DELETE FROM user_tokens \
         WHERE user_id = $1 AND \
         token_hash = $2 AND \
         expires_at > NOW() RETURNING \
         user_id"
      )
      .bind(user_id)
      .bind(old_hash)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    if consumed.is_none() {
      return Err(invalid_token());
    }
    sqlx::query(
      "INSERT INTO user_tokens \
       (user_id, token_hash, \
       expires_at, created_at) VALUES \
       ($1, $2, NOW() + ($3 || ' \
       seconds')::interval, NOW())"
    )
    .bind(user_id)
    .bind(&new_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit()
      .await
      .map_err(database_error)?;
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database pool missing"
      )
    })?;
    let user_id: i64 =
      sqlx::query_scalar(
        "SELECT user_id FROM \
         user_tokens WHERE token_hash \
         = ?1 AND expires_at > \
         datetime('now')"
      )
      .bind(old_hash)
      .fetch_optional(pool)
      .await
      .map_err(database_error)?
      .ok_or_else(invalid_token)?;
    let mut tx = pool
      .begin()
      .await
      .map_err(database_error)?;
    let locked: Option<i64> =
      sqlx::query_scalar(
        "UPDATE users SET \
         password_hash = \
         password_hash WHERE id = ?1 \
         RETURNING id"
      )
      .bind(user_id)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    if locked.is_none() {
      return Err(invalid_token());
    }
    let consumed: Option<i64> =
      sqlx::query_scalar(
        "DELETE FROM user_tokens \
         WHERE user_id = ?1 AND \
         token_hash = ?2 AND \
         expires_at > datetime('now') \
         RETURNING user_id"
      )
      .bind(user_id)
      .bind(old_hash)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    if consumed.is_none() {
      return Err(invalid_token());
    }
    sqlx::query(
      "INSERT INTO user_tokens \
       (user_id, token_hash, \
       expires_at, created_at) VALUES \
       (?1, ?2, datetime('now', '+' \
       || ?3 || ' seconds'), \
       datetime('now'))"
    )
    .bind(user_id)
    .bind(&new_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit()
      .await
      .map_err(database_error)?;
  }

  Ok(token)
}

#[cfg(test)]
mod tests {
  use axum::http::StatusCode;
  use axum::response::IntoResponse;
  use sqlx::sqlite::SqlitePoolOptions;

  use super::*;
  use crate::auth::hash_password;
  use crate::db::recover_user_password;

  async fn fixture() -> AppState {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("sqlite");
    for ddl in [
      "CREATE TABLE users (id INTEGER \
       PRIMARY KEY, username TEXT NOT \
       NULL UNIQUE, password_hash \
       TEXT NOT NULL, is_admin \
       INTEGER NOT NULL DEFAULT 0, \
       created_at TEXT NOT NULL)",
      "CREATE TABLE user_tokens (id \
       INTEGER PRIMARY KEY, user_id \
       INTEGER NOT NULL, token_hash \
       TEXT NOT NULL, expires_at TEXT \
       NOT NULL, created_at TEXT NOT \
       NULL)",
      "CREATE TABLE \
       user_password_resets (id \
       INTEGER PRIMARY KEY, user_id \
       INTEGER NOT NULL, token_hash \
       TEXT NOT NULL, expires_at TEXT \
       NOT NULL, created_at TEXT NOT \
       NULL)"
    ] {
      sqlx::query(ddl)
        .execute(&pool)
        .await
        .expect("fixture schema");
    }
    let hash = hash_password(
      "old-long-password"
    )
    .expect("password hash");
    sqlx::query(
      "INSERT INTO users (id, \
       username, password_hash, \
       created_at) VALUES (1, \
       'reader', ?1, '2026-01-01')"
    )
    .bind(hash)
    .execute(&pool)
    .await
    .expect("fixture user");
    AppState {
      sqlite:            Some(pool),
      postgres:          None,
      fetcher_schema:    None,
      token_ttl_seconds: 3600
    }
  }

  #[tokio::test]
  async fn recovery_invalidates_stale_login_and_rotation()
   {
    let state = fixture().await;
    let pool = state
      .sqlite
      .as_ref()
      .expect("sqlite");
    let old_hash: String =
      sqlx::query_scalar(
        "SELECT password_hash FROM \
         users WHERE id = 1"
      )
      .fetch_one(pool)
      .await
      .expect("original hash");
    let old_token =
      issue_for_verified_login(
        &state, "reader", &old_hash
      )
      .await
      .expect("initial token");
    recover_user_password(
      &state,
      "reader",
      "new-long-password"
    )
    .await
    .expect("recovery");
    let err = issue_for_verified_login(
      &state, "reader", &old_hash
    )
    .await
    .expect_err(
      "stale verified hash rejected"
    );
    assert_eq!(
      err.into_response().status(),
      StatusCode::UNAUTHORIZED
    );
    let err = rotate_existing_token(
      &state,
      &hash_token(&old_token)
    )
    .await
    .expect_err(
      "revoked token cannot rotate"
    );
    assert_eq!(
      err.into_response().status(),
      StatusCode::UNAUTHORIZED
    );
    let remaining: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM \
         user_tokens"
      )
      .fetch_one(pool)
      .await
      .expect(
        "no sessions after recovery"
      );
    assert_eq!(remaining, 0);
    let new_hash: String =
      sqlx::query_scalar(
        "SELECT password_hash FROM \
         users WHERE id = 1"
      )
      .fetch_one(pool)
      .await
      .expect("replacement hash");
    let fresh =
      issue_for_verified_login(
        &state, "reader", &new_hash
      )
      .await
      .expect(
        "new password may log in"
      );
    let rotated =
      rotate_existing_token(
        &state,
        &hash_token(&fresh)
      )
      .await
      .expect("valid token rotates");
    assert_ne!(rotated, fresh);
    let err = rotate_existing_token(
      &state,
      &hash_token(&fresh)
    )
    .await
    .expect_err("old token consumed");
    assert_eq!(
      err.into_response().status(),
      StatusCode::UNAUTHORIZED
    );
  }

  #[tokio::test]
  async fn competing_rotations_create_one_successor()
   {
    let state = fixture().await;
    let pool = state
      .sqlite
      .as_ref()
      .expect("sqlite");
    let hash: String =
      sqlx::query_scalar(
        "SELECT password_hash FROM \
         users WHERE id = 1"
      )
      .fetch_one(pool)
      .await
      .expect("password hash");
    let original =
      issue_for_verified_login(
        &state, "reader", &hash
      )
      .await
      .expect("initial token");
    let old_hash =
      hash_token(&original);
    let mut attempts = Vec::new();
    for _ in 0..8 {
      let state = state.clone();
      let old_hash = old_hash.clone();
      attempts.push(tokio::spawn(
        async move {
          rotate_existing_token(
            &state, &old_hash
          )
          .await
        }
      ));
    }
    let mut succeeded = 0;
    for attempt in attempts {
      match attempt
        .await
        .expect("rotation task")
      {
        | Ok(_) => succeeded += 1,
        | Err(err) => {
          assert_eq!(
            err
              .into_response()
              .status(),
            StatusCode::UNAUTHORIZED
          )
        }
      }
    }
    assert_eq!(succeeded, 1);
    let count: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM \
         user_tokens"
      )
      .fetch_one(pool)
      .await
      .expect("session count");
    assert_eq!(count, 1);
  }
}
