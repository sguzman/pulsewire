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

fn literal_like_pattern(
  query: &str
) -> String {
  let mut pattern = String::new();
  pattern.push('%');
  for ch in query.chars() {
    if ['\\', '%', '_'].contains(&ch) {
      pattern.push('\\');
    }
    pattern.push(ch);
  }
  pattern.push('%');
  pattern
}

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

  let limit = query
    .limit
    .unwrap_or(50)
    .clamp(1, 200) as i64;

  let offset =
    query.offset.unwrap_or(0) as i64;

  let read_filter =
    query.read.as_deref();

  let feed_filter =
    query.feed_id.as_deref();

  let pattern = literal_like_pattern(q);

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
    builder.push(" ESCAPE '\\'");

    builder
      .push(" OR fi.summary ILIKE ");

    builder.push_bind(&pattern);
    builder.push(" ESCAPE '\\'");

    builder.push(
      " OR fi.description ILIKE "
    );

    builder.push_bind(&pattern);
    builder.push(" ESCAPE '\\'");

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

    builder.push_bind(limit + 1);

    builder.push(" OFFSET ");

    builder.push_bind(offset);

    let mut rows = builder
            .build_query_as::<EntrySummary>()
            .fetch_all(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let has_more =
      rows.len() as i64 > limit;
    if has_more {
      rows.truncate(limit as usize);
    }

    let next_cursor = rows
      .iter()
      .map(|row| row.id)
      .max();

    let next_offset = if has_more {
      Some(offset + limit)
    } else {
      None
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
    ") ESCAPE '\\' OR \
     lower(fi.summary) LIKE lower("
  );

  builder.push_bind(&pattern);

  builder.push(
    ") ESCAPE '\\' OR \
     lower(fi.description) LIKE lower("
  );

  builder.push_bind(&pattern);

  // Close both lower(pattern) and the
  // surrounding WHERE (title OR summary
  // OR description) group.
  builder.push(") ESCAPE '\\')");

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

  builder.push_bind(limit + 1);

  builder.push(" OFFSET ");

  builder.push_bind(offset);

  let mut rows = builder
        .build_query_as::<EntrySummary>()
        .fetch_all(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

  let has_more =
    rows.len() as i64 > limit;
  if has_more {
    rows.truncate(limit as usize);
  }

  let next_cursor =
    rows.iter().map(|row| row.id).max();

  let next_offset = if has_more {
    Some(offset + limit)
  } else {
    None
  };

  Ok(Json(EntryListResponse {
    items: rows,
    next_cursor,
    next_offset,
    since: None
  }))
}
