use axum::Json;
use axum::extract::{
  Query,
  State
};
use axum::http::{
  HeaderMap,
  StatusCode
};
use sqlx::{
  Postgres,
  QueryBuilder,
  Sqlite
};

use crate::app_state::AppState;
use crate::auth::auth_user_id;
use crate::db::quote_ident;
use crate::errors::ServerError;
use crate::models::{
  EntryListResponse,
  EntrySummary,
  SearchQuery
};

pub async fn search_entries(
  State(state): State<AppState>,
  headers: HeaderMap,
  Query(query): Query<SearchQuery>
) -> Result<
  Json<EntryListResponse>,
  ServerError
> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let q = query.q.trim();

  if q.is_empty() {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "q required"
    ));
  }

  let limit =
    query.limit.unwrap_or(50).min(200)
      as i64;

  let offset =
    query.offset.unwrap_or(0) as i64;

  let read_filter =
    query.read.as_deref();

  let feed_filter =
    query.feed_id.as_deref();

  let pattern = format!("%{}%", q);

  if let Some(pool) = &state.postgres {
    let schema = state
      .fetcher_schema
      .as_deref()
      .unwrap_or("fetcher");

    let mut builder = QueryBuilder::<
      Postgres
    >::new(
      format!(
      "SELECT fi.id, fi.feed_id, \
       fi.title, fi.link, \
       CAST(EXTRACT(EPOCH FROM \
       fi.published_at) * 1000 AS \
       BIGINT) AS published_at_ms, \
       (es.read_at IS NOT NULL) AS \
       is_read FROM {}.feed_items fi \
       LEFT JOIN entry_states es ON \
       es.item_id = fi.id AND \
       es.user_id = ",
      quote_ident(schema)
    )
    );

    builder.push_bind(user_id);

    builder.push(" WHERE (");

    builder.push("fi.title ILIKE ");

    builder.push_bind(&pattern);

    builder
      .push(" OR fi.summary ILIKE ");

    builder.push_bind(&pattern);

    builder.push(
      " OR fi.description ILIKE "
    );

    builder.push_bind(&pattern);

    builder.push(")");

    if let Some(filter) = read_filter {
      match filter {
        | "read" => {
          builder.push(
            " AND es.read_at IS NOT \
             NULL"
          );
        }
        | "unread" => {
          builder.push(
            " AND es.read_at IS NULL"
          );
        }
        | "all" => {}
        | other => {
          return Err(ServerError::new(
            StatusCode::BAD_REQUEST,
            format!(
              "invalid read filter: \
               {other}"
            )
          ));
        }
      }
    }

    if let Some(feed_id) = feed_filter {
      builder
        .push(" AND fi.feed_id = ");

      builder.push_bind(feed_id);
    }

    builder.push(
      " ORDER BY fi.id DESC LIMIT "
    );

    builder.push_bind(limit);

    builder.push(" OFFSET ");

    builder.push_bind(offset);

    let rows = builder
            .build_query_as::<EntrySummary>()
            .fetch_all(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let next_cursor = rows
      .iter()
      .map(|row| row.id)
      .max();

    let next_offset = if rows.is_empty()
    {
      None
    } else {
      Some(offset + rows.len() as i64)
    };

    return Ok(Json(
      EntryListResponse {
        items: rows,
        next_cursor,
        next_offset,
        since: None
      }
    ));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let mut builder = QueryBuilder::<
    Sqlite
  >::new(
    "SELECT fi.id, fi.feed_id, \
     fi.title, fi.link, \
     fi.published_at_ms, (es.read_at \
     IS NOT NULL) AS is_read FROM \
     feed_items fi LEFT JOIN \
     entry_states es ON es.item_id = \
     fi.id AND es.user_id = "
  );

  builder.push_bind(user_id);

  builder.push(" WHERE (");

  builder.push(
    "lower(fi.title) LIKE lower("
  );

  builder.push_bind(&pattern);

  builder.push(
    ") OR lower(fi.summary) LIKE \
     lower("
  );

  builder.push_bind(&pattern);

  builder.push(
    ") OR lower(fi.description) LIKE \
     lower("
  );

  builder.push_bind(&pattern);

  builder.push(")");

  if let Some(filter) = read_filter {
    match filter {
      | "read" => {
        builder.push(
          " AND es.read_at IS NOT NULL"
        );
      }
      | "unread" => {
        builder.push(
          " AND es.read_at IS NULL"
        );
      }
      | "all" => {}
      | other => {
        return Err(ServerError::new(
          StatusCode::BAD_REQUEST,
          format!(
            "invalid read filter: \
             {other}"
          )
        ));
      }
    }
  }

  if let Some(feed_id) = feed_filter {
    builder.push(" AND fi.feed_id = ");

    builder.push_bind(feed_id);
  }

  builder.push(
    " ORDER BY fi.id DESC LIMIT "
  );

  builder.push_bind(limit);

  builder.push(" OFFSET ");

  builder.push_bind(offset);

  let rows = builder
        .build_query_as::<EntrySummary>()
        .fetch_all(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

  let next_cursor =
    rows.iter().map(|row| row.id).max();

  let next_offset = if rows.is_empty() {
    None
  } else {
    Some(offset + rows.len() as i64)
  };

  Ok(Json(EntryListResponse {
    items: rows,
    next_cursor,
    next_offset,
    since: None
  }))
}
