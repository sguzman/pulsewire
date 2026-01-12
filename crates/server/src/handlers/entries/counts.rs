use axum::Json;
use axum::extract::State;
use axum::http::{
  HeaderMap,
  StatusCode
};

use crate::app_state::AppState;
use crate::auth::auth_user_id;
use crate::db::quote_ident;
use crate::errors::ServerError;
use crate::models::{
  FeedUnreadCount,
  UnreadCountResponse
};

pub async fn unread_count(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<
  Json<UnreadCountResponse>,
  ServerError
> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    let schema = state
      .fetcher_schema
      .as_deref()
      .unwrap_or("fetcher");

    let query = format!(
      "SELECT COUNT(*)::BIGINT FROM \
       {}.feed_items fi JOIN \
       subscriptions s ON s.feed_id = \
       fi.feed_id AND s.user_id = $1 \
       LEFT JOIN entry_states es ON \
       es.item_id = fi.id AND \
       es.user_id = $1 WHERE \
       es.read_at IS NULL",
      quote_ident(schema)
    );

    let count = sqlx::query_scalar::<_, i64>(&query)
            .bind(user_id)
            .fetch_one(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    return Ok(Json(
      UnreadCountResponse {
        count
      }
    ));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let count =
    sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM \
       feed_items fi JOIN \
       subscriptions s ON s.feed_id = \
       fi.feed_id AND s.user_id = ?1 \
       LEFT JOIN entry_states es ON \
       es.item_id = fi.id AND \
       es.user_id = ?1 WHERE \
       es.read_at IS NULL"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string(),
    )
    })?;

  Ok(Json(UnreadCountResponse {
    count
  }))
}

pub async fn feed_unread_counts(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<
  Json<Vec<FeedUnreadCount>>,
  ServerError
> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    let schema = state
      .fetcher_schema
      .as_deref()
      .unwrap_or("fetcher");

    let query = format!(
      "SELECT fi.feed_id, \
       COUNT(*)::BIGINT AS \
       unread_count FROM \
       {}.feed_items fi JOIN \
       subscriptions s ON s.feed_id = \
       fi.feed_id AND s.user_id = $1 \
       LEFT JOIN entry_states es ON \
       es.item_id = fi.id AND \
       es.user_id = $1 WHERE \
       es.read_at IS NULL GROUP BY \
       fi.feed_id ORDER BY fi.feed_id",
      quote_ident(schema)
    );

    let rows = sqlx::query_as::<_, FeedUnreadCount>(&query)
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
    FeedUnreadCount
  >(
    "SELECT fi.feed_id, COUNT(*) AS \
     unread_count FROM feed_items fi \
     JOIN subscriptions s ON \
     s.feed_id = fi.feed_id AND \
     s.user_id = ?1 LEFT JOIN \
     entry_states es ON es.item_id = \
     fi.id AND es.user_id = ?1 WHERE \
     es.read_at IS NULL GROUP BY \
     fi.feed_id ORDER BY fi.feed_id"
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
