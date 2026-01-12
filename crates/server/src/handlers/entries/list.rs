use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use sqlx::{Postgres, QueryBuilder, Sqlite};

use crate::app_state::AppState;
use crate::auth::auth_user_id;
use crate::db::quote_ident;
use crate::errors::ServerError;
use crate::models::{EntryListQuery, EntrySummary};

pub async fn list_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EntryListQuery>,
) -> Result<Json<Vec<EntrySummary>>, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let read_filter = query.read.as_deref();
    let feed_filter = query.feed_id.as_deref();
    let since = query.since;

    if let Some(pool) = &state.postgres {
        let schema = state.fetcher_schema.as_deref().unwrap_or("fetcher");
        let mut builder = QueryBuilder::<Postgres>::new(format!(
            "SELECT fi.id, fi.feed_id, fi.title, fi.link, \
            CAST(EXTRACT(EPOCH FROM fi.published_at) * 1000 AS BIGINT) AS published_at_ms, \
            (es.read_at IS NOT NULL) AS is_read \
            FROM {}.feed_items fi \
            LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = ",
            quote_ident(schema)
        ));
        builder.push_bind(user_id);
        builder.push(" WHERE 1=1");

        if let Some(filter) = read_filter {
            match filter {
                "read" => {
                    builder.push(" AND es.read_at IS NOT NULL");
                }
                "unread" => {
                    builder.push(" AND es.read_at IS NULL");
                }
                "all" => {}
                other => {
                    return Err(ServerError::new(
                        StatusCode::BAD_REQUEST,
                        format!("invalid read filter: {other}"),
                    ));
                }
            }
        }

        if let Some(feed_id) = feed_filter {
            builder.push(" AND fi.feed_id = ");
            builder.push_bind(feed_id);
        }

        if let Some(since_id) = since {
            builder.push(" AND fi.id > ");
            builder.push_bind(since_id);
        }

        builder.push(" ORDER BY fi.id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<EntrySummary>()
            .fetch_all(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(rows));
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT fi.id, fi.feed_id, fi.title, fi.link, \
        fi.published_at_ms, \
        (es.read_at IS NOT NULL) AS is_read \
        FROM feed_items fi \
        LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = ",
    );
    builder.push_bind(user_id);
    builder.push(" WHERE 1=1");

    if let Some(filter) = read_filter {
        match filter {
            "read" => {
                builder.push(" AND es.read_at IS NOT NULL");
            }
            "unread" => {
                builder.push(" AND es.read_at IS NULL");
            }
            "all" => {}
            other => {
                return Err(ServerError::new(
                    StatusCode::BAD_REQUEST,
                    format!("invalid read filter: {other}"),
                ));
            }
        }
    }

    if let Some(feed_id) = feed_filter {
        builder.push(" AND fi.feed_id = ");
        builder.push_bind(feed_id);
    }

    if let Some(since_id) = since {
        builder.push(" AND fi.id > ");
        builder.push_bind(since_id);
    }

    builder.push(" ORDER BY fi.id DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let rows = builder
        .build_query_as::<EntrySummary>()
        .fetch_all(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn list_feed_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(feed_id): AxumPath<String>,
    Query(query): Query<EntryListQuery>,
) -> Result<Json<Vec<EntrySummary>>, ServerError> {
    let mut query = query;
    query.feed_id = Some(feed_id);
    list_entries(State(state), headers, Query(query)).await
}
