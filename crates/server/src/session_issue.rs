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

fn invalid_credentials() -> ServerError {
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
  let ttl = state.token_ttl_seconds as i64;

  if let Some(pool) = &state.postgres {
    let mut tx =
      pool.begin().await.map_err(database_error)?;
    let current: Option<(i64, String)> =
      sqlx::query_as(
        "SELECT id, password_hash FROM users \
         WHERE username = $1 FOR UPDATE"
      )
      .bind(username)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    let (user_id, current_hash) =
      current.ok_or_else(invalid_credentials)?;
    if current_hash != verified_hash {
      return Err(invalid_credentials());
    }
    sqlx::query(
      "INSERT INTO user_tokens \
       (user_id, token_hash, expires_at, created_at) \
       VALUES ($1, $2, \
       NOW() + ($3 || ' seconds')::interval, NOW())"
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database pool missing"
      )
    })?;
    let mut tx =
      pool.begin().await.map_err(database_error)?;
    // A SQLite UPDATE acquires the write
    // lock before the hash is read. A
    // deferred SELECT would allow a
    // stale snapshot/race with recovery.
    let current: Option<(i64, String)> =
      sqlx::query_as(
        "UPDATE users SET password_hash = password_hash \
         WHERE username = ?1 RETURNING id, password_hash"
      )
      .bind(username)
      .fetch_optional(&mut *tx)
      .await
      .map_err(database_error)?;
    let (user_id, current_hash) =
      current.ok_or_else(invalid_credentials)?;
    if current_hash != verified_hash {
      return Err(invalid_credentials());
    }
    sqlx::query(
      "INSERT INTO user_tokens \
       (user_id, token_hash, expires_at, created_at) \
       VALUES (?1, ?2, \
       datetime('now', '+' || ?3 || ' seconds'), \
       datetime('now'))"
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
  }

  Ok(token)
}

/// Consumes the original bearer and
/// creates a replacement atomically.
/// The preliminary lookup is advisory:
/// the old token is checked again *after*
/// acquiring the user lock, which also
/// serializes it with account recovery.
pub(crate) async fn rotate_existing_token(
  state: &AppState,
  old_hash: &str
) -> Result<String, ServerError> {
  let token = generate_token();
  let new_hash = hash_token(&token);
  let ttl = state.token_ttl_seconds as i64;

  if let Some(pool) = &state.postgres {
    let user_id: i64 = sqlx::query_scalar(
      "SELECT user_id FROM user_tokens \
       WHERE token_hash = $1 AND expires_at > NOW()"
    )
    .bind(old_hash)
    .fetch_optional(pool)
    .await
    .map_err(database_error)?
    .ok_or_else(invalid_token)?;
    let mut tx =
      pool.begin().await.map_err(database_error)?;
    let locked: Option<i64> = sqlx::query_scalar(
      "SELECT id FROM users WHERE id = $1 FOR UPDATE"
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(database_error)?;
    if locked.is_none() {
      return Err(invalid_token());
    }
    let consumed: Option<i64> = sqlx::query_scalar(
      "DELETE FROM user_tokens \
       WHERE user_id = $1 AND token_hash = $2 \
       AND expires_at > NOW() RETURNING user_id"
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
       (user_id, token_hash, expires_at, created_at) \
       VALUES ($1, $2, \
       NOW() + ($3 || ' seconds')::interval, NOW())"
    )
    .bind(user_id)
    .bind(&new_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database pool missing"
      )
    })?;
    let user_id: i64 = sqlx::query_scalar(
      "SELECT user_id FROM user_tokens \
       WHERE token_hash = ?1 \
       AND expires_at > datetime('now')"
    )
    .bind(old_hash)
    .fetch_optional(pool)
    .await
    .map_err(database_error)?
    .ok_or_else(invalid_token)?;
    let mut tx =
      pool.begin().await.map_err(database_error)?;
    let locked: Option<i64> = sqlx::query_scalar(
      "UPDATE users SET password_hash = password_hash \
       WHERE id = ?1 RETURNING id"
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(database_error)?;
    if locked.is_none() {
      return Err(invalid_token());
    }
    let consumed: Option<i64> = sqlx::query_scalar(
      "DELETE FROM user_tokens \
       WHERE user_id = ?1 AND token_hash = ?2 \
       AND expires_at > datetime('now') \
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
       (user_id, token_hash, expires_at, created_at) \
       VALUES (?1, ?2, \
       datetime('now', '+' || ?3 || ' seconds'), \
       datetime('now'))"
    )
    .bind(user_id)
    .bind(&new_hash)
    .bind(ttl)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
  }

  Ok(token)
}
