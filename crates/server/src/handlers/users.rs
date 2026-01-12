use axum::{extract::State, http::StatusCode, Json};

use crate::app_state::AppState;
use crate::auth::hash_password;
use crate::errors::{map_db_error, ServerError};
use crate::models::{CreateUserRequest, UserResponse};

pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, ServerError> {
    let username = payload.username.trim();
    let password = payload.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err(ServerError::new(
            StatusCode::BAD_REQUEST,
            "username and password required",
        ));
    }

    let password_hash =
        hash_password(password).map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let user_id = if let Some(pool) = &state.postgres {
        let row = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (username, password_hash, created_at) VALUES ($1, $2, NOW()) RETURNING id",
        )
        .bind(username)
        .bind(&password_hash)
        .fetch_one(pool)
        .await
        .map_err(|e| map_db_error(e, "user create failed"))?;
        row
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query(
            "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, datetime('now'))",
        )
        .bind(username)
        .bind(&password_hash)
        .execute(pool)
        .await
        .map_err(|e| map_db_error(e, "user create failed"))?;
        sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(UserResponse {
        id: user_id,
        username: username.to_string(),
    }))
}
