use axum::Json;
use axum::extract::State;
use axum::http::{
  HeaderMap,
  StatusCode
};

use crate::app_state::AppState;
use crate::auth::auth_user_id;
use crate::errors::ServerError;
use crate::models::EntryBatchRequest;

pub async fn mark_entries_read(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<
    EntryBatchRequest
  >
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if payload.item_ids.is_empty() {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "item_ids required"
    ));
  }

  if let Some(pool) = &state.postgres {
    sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at) \
            SELECT $1, UNNEST($2::BIGINT[]), NOW() \
            ON CONFLICT (user_id, item_id) DO UPDATE SET read_at = EXCLUDED.read_at",
        )
        .bind(user_id)
        .bind(&payload.item_ids)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    return Ok(StatusCode::NO_CONTENT);
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

  for item_id in payload.item_ids {
    sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at) VALUES (?1, ?2, datetime('now')) \
            ON CONFLICT(user_id, item_id) DO UPDATE SET read_at = excluded.read_at",
        )
        .bind(user_id)
        .bind(item_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  tx.commit().await.map_err(|e| {
    ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string()
    )
  })?;

  Ok(StatusCode::NO_CONTENT)
}

pub async fn mark_entries_unread(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<
    EntryBatchRequest
  >
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if payload.item_ids.is_empty() {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "item_ids required"
    ));
  }

  if let Some(pool) = &state.postgres {
    sqlx::query("DELETE FROM entry_states WHERE user_id = $1 AND item_id = ANY($2)")
            .bind(user_id)
            .bind(&payload.item_ids)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    return Ok(StatusCode::NO_CONTENT);
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

  for item_id in payload.item_ids {
    sqlx::query("DELETE FROM entry_states WHERE user_id = ?1 AND item_id = ?2")
            .bind(user_id)
            .bind(item_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  tx.commit().await.map_err(|e| {
    ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string()
    )
  })?;

  Ok(StatusCode::NO_CONTENT)
}
