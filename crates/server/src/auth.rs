use argon2::{
    password_hash::{rand_core::OsRng, rand_core::RngCore, SaltString},
    Argon2,
    PasswordHash,
    PasswordHasher,
    PasswordVerifier,
};
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};

use crate::app_state::AppState;
use crate::errors::ServerError;

pub async fn auth_user_id(state: &AppState, headers: &HeaderMap) -> Result<i64, ServerError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_token(&token);

    if let Some(pool) = &state.postgres {
        let id = sqlx::query_scalar::<_, i64>(
            "SELECT user_id FROM user_tokens WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(axum::http::StatusCode::UNAUTHORIZED, "invalid token"))?;
        return Ok(id);
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let id = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM user_tokens WHERE token_hash = ?1 AND expires_at > datetime('now')",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| ServerError::new(axum::http::StatusCode::UNAUTHORIZED, "invalid token"))?;
    Ok(id)
}

pub fn bearer_token(headers: &HeaderMap) -> Result<String, ServerError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let token = value.strip_prefix("Bearer ").unwrap_or("").trim();
    if token.is_empty() {
        return Err(ServerError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "missing bearer token",
        ));
    }
    Ok(token.to_string())
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("password hash error: {e}"))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(hash: &str, password: &str) -> Result<(), String> {
    let parsed = PasswordHash::new(hash).map_err(|e| format!("password hash parse error: {e}"))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|e| format!("password verify error: {e}"))
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
