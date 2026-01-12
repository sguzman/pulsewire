use axum::{http::StatusCode, extract::State, Json};

use crate::app_state::AppState;
use crate::db::quote_ident;
use crate::errors::ServerError;
use crate::models::FeedSummary;

pub async fn list_feeds(
    State(state): State<AppState>,
) -> Result<Json<Vec<FeedSummary>>, ServerError> {
    if let Some(pool) = &state.postgres {
        let schema = state.fetcher_schema.as_deref().unwrap_or("fetcher");
        let query = format!(
            "SELECT id, url, domain, category, base_poll_seconds FROM {}.feeds ORDER BY id",
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
    let rows = sqlx::query_as::<_, FeedSummary>(
        "SELECT id, url, domain, category, base_poll_seconds FROM feeds ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        ServerError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("feeds query failed: {e}"),
        )
    })?;
    Ok(Json(rows))
}
