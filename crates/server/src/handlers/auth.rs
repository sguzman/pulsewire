use axum::Json;
use axum::extract::{
  Path as AxumPath,
  State
};
use axum::http::{
  HeaderMap,
  StatusCode
};

use crate::app_state::AppState;
use crate::auth::{
  bearer_token,
  hash_token,
  verify_password
};
use crate::errors::ServerError;
use crate::login_throttle;
use crate::models::{
  LoginRequest,
  TokenResponse,
  TokenSummary
};

pub async fn login(
  State(state): State<AppState>,
  Json(payload): Json<LoginRequest>
) -> Result<
  Json<TokenResponse>,
  ServerError
> {
  let username =
    payload.username.trim();

  let password = payload.password.as_str();

  if username.is_empty()
    || password.is_empty()
  {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "username and password required"
    ));
  }

  login_throttle::reserve_attempt(
    &state, username
  )
  .await?;

  let (_user_id, password_hash) =
    if let Some(pool) = &state.postgres
    {
      sqlx::query_as::<_, (i64, String)>(
            "SELECT id, password_hash FROM users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?
    } else {
      let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

      sqlx::query_as::<_, (i64, String)>(
            "SELECT id, password_hash FROM users WHERE username = ?1",
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?
    };

  verify_password(
    &password_hash,
    password
  )
  .map_err(|_| {
    ServerError::new(
      StatusCode::UNAUTHORIZED,
      "invalid credentials"
    )
  })?;

  let token =
    crate::session_issue::issue_for_verified_login(
      &state,
      username,
      &password_hash
    )
    .await?;

  login_throttle::clear_attempts(
    &state, username
  )
  .await?;

  Ok(Json(TokenResponse {
    token,
    token_type: "bearer".to_string(),
    expires_in: state.token_ttl_seconds
  }))
}

pub async fn logout(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<StatusCode, ServerError> {
  let token = bearer_token(&headers)?;

  let token_hash = hash_token(&token);

  if let Some(pool) = &state.postgres {
    sqlx::query("DELETE FROM user_tokens WHERE token_hash = $1")
            .bind(&token_hash)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query("DELETE FROM user_tokens WHERE token_hash = ?1")
            .bind(&token_hash)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  Ok(StatusCode::NO_CONTENT)
}

pub async fn rotate_token(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<
  Json<TokenResponse>,
  ServerError
> {
  let token = bearer_token(&headers)?;
  let token_hash = hash_token(&token);

  let new_token =
    crate::session_issue::rotate_existing_token(
      &state,
      &token_hash
    )
    .await?;

  Ok(Json(TokenResponse {
    token:      new_token,
    token_type: "bearer".to_string(),
    expires_in: state.token_ttl_seconds
  }))
}

pub async fn list_tokens(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<
  Json<Vec<TokenSummary>>,
  ServerError
> {
  let user_id =
    crate::auth::auth_user_id(
      &state, &headers
    )
    .await?;

  if let Some(pool) = &state.postgres {
    let rows = sqlx::query_as::<_, TokenSummary>(
            "SELECT id, expires_at::text AS expires_at, created_at::text AS created_at FROM user_tokens WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    return Ok(Json(rows));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let rows =
    sqlx::query_as::<_, TokenSummary>(
      "SELECT id, expires_at, \
       created_at FROM user_tokens \
       WHERE user_id = ?1 ORDER BY \
       created_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string(),
    )
    })?;

  Ok(Json(rows))
}

pub async fn revoke_token(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(token_id): AxumPath<i64>
) -> Result<StatusCode, ServerError> {
  let user_id =
    crate::auth::auth_user_id(
      &state, &headers
    )
    .await?;

  let rows = if let Some(pool) =
    &state.postgres
  {
    sqlx::query("DELETE FROM user_tokens WHERE id = $1 AND user_id = $2")
            .bind(token_id)
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

    sqlx::query("DELETE FROM user_tokens WHERE id = ?1 AND user_id = ?2")
            .bind(token_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
  };

  if rows == 0 {
    return Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "token not found"
    ));
  }

  Ok(StatusCode::NO_CONTENT)
}
