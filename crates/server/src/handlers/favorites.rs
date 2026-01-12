use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::app_state::AppState;
use crate::auth::auth_user_id;
use crate::db::quote_ident;
use crate::errors::{map_db_error, ServerError};
use crate::models::{EntrySummary, FavoriteListQuery, FavoriteRequest};

pub async fn list_favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<FavoriteListQuery>,
) -> Result<Json<Vec<EntrySummary>>, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    if let Some(pool) = &state.postgres {
        let schema = state.fetcher_schema.as_deref().unwrap_or("fetcher");
        let query = format!(
            "SELECT fi.id, fi.feed_id, fi.title, fi.link, \
            CAST(EXTRACT(EPOCH FROM fi.published_at) * 1000 AS BIGINT) AS published_at_ms, \
            (es.read_at IS NOT NULL) AS is_read \
            FROM favorites fav \
            JOIN {}.feed_items fi ON fi.id = fav.item_id \
            LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = $1 \
            WHERE fav.user_id = $1 \
            ORDER BY fav.created_at DESC \
            LIMIT $2 OFFSET $3",
            quote_ident(schema)
        );
        let rows = sqlx::query_as::<_, EntrySummary>(&query)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(rows));
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let rows = sqlx::query_as::<_, EntrySummary>(
        "SELECT fi.id, fi.feed_id, fi.title, fi.link, \
        fi.published_at_ms, \
        (es.read_at IS NOT NULL) AS is_read \
        FROM favorites fav \
        JOIN feed_items fi ON fi.id = fav.item_id \
        LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = ?1 \
        WHERE fav.user_id = ?1 \
        ORDER BY fav.created_at DESC \
        LIMIT ?2 OFFSET ?3",
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn create_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FavoriteRequest>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;

    if let Some(pool) = &state.postgres {
        sqlx::query(
            "INSERT INTO favorites (user_id, item_id, created_at) VALUES ($1, $2, NOW())",
        )
        .bind(user_id)
        .bind(payload.item_id)
        .execute(pool)
        .await
        .map_err(|e| map_db_error(e, "favorite create failed"))?;
        return Ok(StatusCode::CREATED);
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    sqlx::query(
        "INSERT INTO favorites (user_id, item_id, created_at) VALUES (?1, ?2, datetime('now'))",
    )
    .bind(user_id)
    .bind(payload.item_id)
    .execute(pool)
    .await
    .map_err(|e| map_db_error(e, "favorite create failed"))?;
    Ok(StatusCode::CREATED)
}

pub async fn delete_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(item_id): AxumPath<i64>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;

    let rows = if let Some(pool) = &state.postgres {
        sqlx::query("DELETE FROM favorites WHERE user_id = $1 AND item_id = $2")
            .bind(user_id)
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query("DELETE FROM favorites WHERE user_id = ?1 AND item_id = ?2")
            .bind(user_id)
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
    };

    if rows == 0 {
        return Err(ServerError::new(StatusCode::NOT_FOUND, "favorite not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}
