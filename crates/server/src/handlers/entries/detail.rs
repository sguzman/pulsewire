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
use crate::db::quote_ident;
use crate::errors::ServerError;
use crate::models::EntryDetail;

pub async fn entry_detail(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(item_id): AxumPath<i64>
) -> Result<
  Json<EntryDetail>,
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
      "SELECT fi.id, fi.feed_id, \
       fi.title, fi.link, fi.guid, \
       CAST(EXTRACT(EPOCH FROM \
       fi.published_at) * 1000 AS \
       BIGINT) AS published_at_ms, \
       fi.category, fi.description, \
       fi.summary, (es.read_at IS NOT \
       NULL) AS is_read FROM \
       {}.feed_items fi LEFT JOIN \
       entry_states es ON es.item_id \
       = fi.id AND es.user_id = $1 \
       WHERE fi.id = $2",
      quote_ident(schema)
    );

    let row = sqlx::query_as::<_, EntryDetail>(&query)
            .bind(user_id)
            .bind(item_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "entry not found"))?;

    return Ok(Json(row));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let row =
    sqlx::query_as::<_, EntryDetail>(
      "SELECT fi.id, fi.feed_id, \
       fi.title, fi.link, fi.guid, \
       fi.published_at_ms, \
       fi.category, fi.description, \
       fi.summary, (es.read_at IS NOT \
       NULL) AS is_read FROM \
       feed_items fi LEFT JOIN \
       entry_states es ON es.item_id \
       = fi.id AND es.user_id = ?1 \
       WHERE fi.id = ?2"
    )
    .bind(user_id)
    .bind(item_id)
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
        "entry not found"
      )
    })?;

  Ok(Json(row))
}
