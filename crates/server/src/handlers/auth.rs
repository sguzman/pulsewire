use axum::{extract::State, http::{HeaderMap, StatusCode}, Json};

use crate::app_state::AppState;
use crate::auth::{bearer_token, generate_token, hash_token, verify_password};
use crate::errors::ServerError;
use crate::models::{LoginRequest, TokenResponse};

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ServerError> {
    let username = payload.username.trim();
    let password = payload.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err(ServerError::new(
            StatusCode::BAD_REQUEST,
            "username and password required",
        ));
    }

    let (user_id, password_hash) = if let Some(pool) = &state.postgres {
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

    verify_password(&password_hash, password)
        .map_err(|_| ServerError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?;

    let token = generate_token();
    let token_hash = hash_token(&token);
    let ttl = state.token_ttl_seconds as i64;

    if let Some(pool) = &state.postgres {
        sqlx::query(
            "INSERT INTO user_tokens (user_id, token_hash, expires_at, created_at) VALUES ($1, $2, NOW() + ($3 || ' seconds')::interval, NOW())",
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
            "INSERT INTO user_tokens (user_id, token_hash, expires_at, created_at) VALUES (?1, ?2, datetime('now', '+' || ?3 || ' seconds'), datetime('now'))",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(ttl)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(TokenResponse {
        token,
        token_type: "bearer".to_string(),
        expires_in: state.token_ttl_seconds,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
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
