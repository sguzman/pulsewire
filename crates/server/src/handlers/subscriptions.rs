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
use crate::auth::auth_user_id;
use crate::errors::{
  ServerError,
  map_db_error
};
use crate::models::{
  SubscriptionRequest,
  SubscriptionRow
};

pub async fn list_subscriptions(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<
  Json<Vec<SubscriptionRow>>,
  ServerError
> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    let rows = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT feed_id FROM subscriptions WHERE user_id = $1 ORDER BY feed_id",
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

  let rows = sqlx::query_as::<
    _,
    SubscriptionRow
  >(
    "SELECT feed_id FROM \
     subscriptions WHERE user_id = ?1 \
     ORDER BY feed_id"
  )
  .bind(user_id)
  .fetch_all(pool)
  .await
  .map_err(|e| {
    ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string()
    )
  })?;

  Ok(Json(rows))
}

pub async fn create_subscription(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<
    SubscriptionRequest
  >
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let feed_id = payload.feed_id.trim();

  if feed_id.is_empty() {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "feed_id required"
    ));
  }

  if let Some(pool) = &state.postgres {
    sqlx::query(
      "INSERT INTO subscriptions \
       (user_id, feed_id, created_at) \
       VALUES ($1, $2, NOW())"
    )
    .bind(user_id)
    .bind(feed_id)
    .execute(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "subscription create failed"
      )
    })?;

    return Ok(StatusCode::CREATED);
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  sqlx::query(
    "INSERT INTO subscriptions \
     (user_id, feed_id, created_at) \
     VALUES (?1, ?2, datetime('now'))"
  )
  .bind(user_id)
  .bind(feed_id)
  .execute(pool)
  .await
  .map_err(|e| {
    map_db_error(
      e,
      "subscription create failed"
    )
  })?;

  Ok(StatusCode::CREATED)
}

pub async fn delete_subscription(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(feed_id): AxumPath<String>
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    sqlx::query("DELETE FROM subscriptions WHERE user_id = $1 AND feed_id = $2")
            .bind(user_id)
            .bind(&feed_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    return Ok(StatusCode::NO_CONTENT);
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  sqlx::query(
    "DELETE FROM subscriptions WHERE \
     user_id = ?1 AND feed_id = ?2"
  )
  .bind(user_id)
  .bind(&feed_id)
  .execute(pool)
  .await
  .map_err(|e| {
    ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string()
    )
  })?;

  Ok(StatusCode::NO_CONTENT)
}
