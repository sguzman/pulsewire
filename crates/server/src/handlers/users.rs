use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::app_state::AppState;
use crate::auth::{
  auth_user_id,
  generate_token,
  hash_password,
  hash_token,
  verify_password
};
use crate::errors::{
  ServerError,
  map_db_error
};
use crate::models::{
  CreateUserRequest,
  PasswordChangeRequest,
  PasswordResetConfirm,
  PasswordResetRequest,
  PasswordResetResponse,
  UserResponse
};

pub async fn create_user(
  State(state): State<AppState>,
  Json(payload): Json<
    CreateUserRequest
  >
) -> Result<
  Json<UserResponse>,
  ServerError
> {
  let username =
    payload.username.trim();

  let password =
    payload.password.trim();

  if username.is_empty()
    || password.is_empty()
  {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "username and password required"
    ));
  }

  let password_hash = hash_password(
    password
  )
  .map_err(|e| {
    ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e
    )
  })?;

  let user_id = if let Some(pool) =
    &state.postgres
  {
    sqlx::query_scalar::<_, i64>(
      "INSERT INTO users (username, \
       password_hash, created_at) \
       VALUES ($1, $2, NOW()) \
       RETURNING id"
    )
    .bind(username)
    .bind(&password_hash)
    .fetch_one(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "user create failed"
      )
    })?
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query(
      "INSERT INTO users (username, \
       password_hash, created_at) \
       VALUES (?1, ?2, \
       datetime('now'))"
    )
    .bind(username)
    .bind(&password_hash)
    .execute(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "user create failed"
      )
    })?;

    sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
  };

  Ok(Json(UserResponse {
    id:       user_id,
    username: username.to_string()
  }))
}

pub async fn change_password(
  State(state): State<AppState>,
  headers: axum::http::HeaderMap,
  Json(payload): Json<
    PasswordChangeRequest
  >
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let current_password =
    payload.current_password.trim();

  let new_password =
    payload.new_password.trim();

  if current_password.is_empty()
    || new_password.is_empty()
  {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "current_password and \
       new_password required"
    ));
  }

  let password_hash = if let Some(
    pool
  ) =
    &state.postgres
  {
    sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "user not found"))?
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "user not found"))?
  };

  verify_password(
    &password_hash,
    current_password
  )
  .map_err(|_| {
    ServerError::new(
      StatusCode::UNAUTHORIZED,
      "invalid credentials"
    )
  })?;

  let new_hash =
    hash_password(new_password)
      .map_err(|e| {
        ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e,
    )
      })?;

  if let Some(pool) = &state.postgres {
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(&new_hash)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
            .bind(&new_hash)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_user(
  State(state): State<AppState>,
  headers: axum::http::HeaderMap
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let rows = if let Some(pool) =
    &state.postgres
  {
    sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
  };

  if rows == 0 {
    return Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "user not found"
    ));
  }

  Ok(StatusCode::NO_CONTENT)
}

pub async fn request_password_reset(
  State(state): State<AppState>,
  Json(payload): Json<
    PasswordResetRequest
  >
) -> Result<
  Json<PasswordResetResponse>,
  ServerError
> {
  let username =
    payload.username.trim();

  if username.is_empty() {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "username required"
    ));
  }

  let user_id = if let Some(pool) =
    &state.postgres
  {
    sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "user not found"))?
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE username = ?1")
            .bind(username)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "user not found"))?
  };

  let token = generate_token();

  let token_hash = hash_token(&token);

  let ttl = 3600i64;

  if let Some(pool) = &state.postgres {
    sqlx::query(
            "INSERT INTO user_password_resets (user_id, token_hash, expires_at, created_at) VALUES ($1, $2, NOW() + ($3 || ' seconds')::interval, NOW())",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(ttl)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query(
            "INSERT INTO user_password_resets (user_id, token_hash, expires_at, created_at) VALUES (?1, ?2, datetime('now', '+' || ?3 || ' seconds'), datetime('now'))",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(ttl)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  Ok(Json(PasswordResetResponse {
    reset_token: token,
    expires_in:  ttl as u64
  }))
}

pub async fn confirm_password_reset(
  State(state): State<AppState>,
  Json(payload): Json<
    PasswordResetConfirm
  >
) -> Result<StatusCode, ServerError> {
  let token = payload.token.trim();

  let new_password =
    payload.new_password.trim();

  if token.is_empty()
    || new_password.is_empty()
  {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "token and new_password required"
    ));
  }

  let token_hash = hash_token(token);

  let user_id = if let Some(pool) =
    &state.postgres
  {
    sqlx::query_scalar::<_, i64>(
            "SELECT user_id FROM user_password_resets WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid token"))?
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query_scalar::<_, i64>(
            "SELECT user_id FROM user_password_resets WHERE token_hash = ?1 AND expires_at > datetime('now')",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid token"))?
  };

  let new_hash =
    hash_password(new_password)
      .map_err(|e| {
        ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e,
    )
      })?;

  if let Some(pool) = &state.postgres {
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(&new_hash)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM user_password_resets WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
            .bind(&new_hash)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM user_password_resets WHERE user_id = ?1")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  Ok(StatusCode::NO_CONTENT)
}
