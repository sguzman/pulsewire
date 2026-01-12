use axum::extract::{
  Path as AxumPath,
  State
};
use axum::http::{
  HeaderMap,
  StatusCode
};

use crate::app_state::AppState;
use crate::auth::auth_user_id;
use crate::errors::ServerError;

pub async fn read_state(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(item_id): AxumPath<i64>
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let exists = if let Some(pool) =
    &state.postgres
  {
    sqlx::query_scalar::<_, i64>(
            "SELECT 1::BIGINT FROM entry_states WHERE user_id = $1 AND item_id = $2 AND read_at IS NOT NULL",
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some()
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM entry_states WHERE user_id = ?1 AND item_id = ?2 AND read_at IS NOT NULL",
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some()
  };

  if exists {
    Ok(StatusCode::NO_CONTENT)
  } else {
    Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "unread"
    ))
  }
}

pub async fn mark_read(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(item_id): AxumPath<i64>
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at) VALUES ($1, $2, NOW()) \
            ON CONFLICT (user_id, item_id) DO UPDATE SET read_at = EXCLUDED.read_at",
        )
        .bind(user_id)
        .bind(item_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at) VALUES (?1, ?2, datetime('now')) \
            ON CONFLICT(user_id, item_id) DO UPDATE SET read_at = excluded.read_at",
        )
        .bind(user_id)
        .bind(item_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  Ok(StatusCode::NO_CONTENT)
}

pub async fn mark_unread(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(item_id): AxumPath<i64>
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    sqlx::query("DELETE FROM entry_states WHERE user_id = $1 AND item_id = $2")
            .bind(user_id)
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query("DELETE FROM entry_states WHERE user_id = ?1 AND item_id = ?2")
            .bind(user_id)
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  Ok(StatusCode::NO_CONTENT)
}
