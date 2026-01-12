use axum::Json;
use axum::extract::{
  Path as AxumPath,
  State
};
use axum::http::StatusCode;

use crate::app_state::AppState;
use crate::db::quote_ident;
use crate::errors::ServerError;
use crate::models::{
  FeedDetail,
  FeedSummary
};

pub async fn feed_detail(
  State(state): State<AppState>,
  AxumPath(feed_id): AxumPath<String>
) -> Result<Json<FeedDetail>, ServerError>
{
  if let Some(pool) = &state.postgres {
    let schema = state
      .fetcher_schema
      .as_deref()
      .unwrap_or("fetcher");

    let query = format!(
            "SELECT id, url, domain, category, base_poll_seconds,             CAST(EXTRACT(EPOCH FROM created_at) * 1000 AS BIGINT) AS created_at_ms             FROM {}.feeds WHERE id = $1",
            quote_ident(schema)
        );

    let row = sqlx::query_as::<_, FeedDetail>(&query)
            .bind(&feed_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "feed not found"))?;

    return Ok(Json(row));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let row =
    sqlx::query_as::<_, FeedDetail>(
      "SELECT id, url, domain, \
       category, base_poll_seconds, \
       created_at_ms FROM feeds WHERE \
       id = ?1"
    )
    .bind(&feed_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string(),
    )
    })?
    .ok_or_else(|| {
      ServerError::new(
        StatusCode::NOT_FOUND,
        "feed not found"
      )
    })?;

  Ok(Json(row))
}

pub async fn list_feeds(
  State(state): State<AppState>
) -> Result<
  Json<Vec<FeedSummary>>,
  ServerError
> {
  if let Some(pool) = &state.postgres {
    let schema = state
      .fetcher_schema
      .as_deref()
      .unwrap_or("fetcher");

    let query = format!(
      "SELECT id, url, domain, \
       category, base_poll_seconds \
       FROM {}.feeds ORDER BY id",
      quote_ident(schema)
    );

    let rows = sqlx::query_as::<_, FeedSummary>(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                ServerError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("feeds query failed: {e}"),
                )
            })?;

    return Ok(Json(rows));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let rows =
    sqlx::query_as::<_, FeedSummary>(
      "SELECT id, url, domain, \
       category, base_poll_seconds \
       FROM feeds ORDER BY id"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      format!(
        "feeds query failed: {e}"
      ),
    )
    })?;

  Ok(Json(rows))
}
