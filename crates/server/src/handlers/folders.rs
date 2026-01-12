use axum::Json;
use axum::extract::{
  Path as AxumPath,
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
use crate::errors::{
  ServerError,
  map_db_error
};
use crate::models::{
  EntryListResponse,
  EntrySummary,
  FolderCreateRequest,
  FolderEntriesQuery,
  FolderFeedRequest,
  FolderFeedRow,
  FolderFeedUnreadCount,
  FolderRow,
  FolderUnreadCount,
  FolderUpdateRequest
};

pub async fn list_folders(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<
  Json<Vec<FolderRow>>,
  ServerError
> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    let rows = sqlx::query_as::<_, FolderRow>(
            "SELECT id, name FROM folders WHERE user_id = $1 ORDER BY name",
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

  let rows =
    sqlx::query_as::<_, FolderRow>(
      "SELECT id, name FROM folders \
       WHERE user_id = ?1 ORDER BY \
       name"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string(),
    )
    })?;

  Ok(Json(rows))
}

pub async fn create_folder(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<
    FolderCreateRequest
  >
) -> Result<Json<FolderRow>, ServerError>
{
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let name = payload.name.trim();

  if name.is_empty() {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "name required"
    ));
  }

  if let Some(pool) = &state.postgres {
    let row =
      sqlx::query_as::<_, FolderRow>(
        "INSERT INTO folders \
         (user_id, name, created_at) \
         VALUES ($1, $2, NOW()) \
         RETURNING id, name"
      )
      .bind(user_id)
      .bind(name)
      .fetch_one(pool)
      .await
      .map_err(|e| {
        map_db_error(
          e,
          "folder create failed"
        )
      })?;

    return Ok(Json(row));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  sqlx::query(
    "INSERT INTO folders (user_id, \
     name, created_at) VALUES (?1, \
     ?2, datetime('now'))"
  )
  .bind(user_id)
  .bind(name)
  .execute(pool)
  .await
  .map_err(|e| {
    map_db_error(
      e,
      "folder create failed"
    )
  })?;

  let row =
    sqlx::query_as::<_, FolderRow>(
      "SELECT id, name FROM folders \
       WHERE id = last_insert_rowid()"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string(),
    )
    })?;

  Ok(Json(row))
}

pub async fn update_folder(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(folder_id): AxumPath<i64>,
  Json(payload): Json<
    FolderUpdateRequest
  >
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let name = payload.name.trim();

  if name.is_empty() {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "name required"
    ));
  }

  let rows = if let Some(pool) =
    &state.postgres
  {
    sqlx::query(
      "UPDATE folders SET name = $1 \
       WHERE id = $2 AND user_id = $3"
    )
    .bind(name)
    .bind(folder_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "folder update failed"
      )
    })?
    .rows_affected()
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query(
      "UPDATE folders SET name = ?1 \
       WHERE id = ?2 AND user_id = ?3"
    )
    .bind(name)
    .bind(folder_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "folder update failed"
      )
    })?
    .rows_affected()
  };

  if rows == 0 {
    return Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "folder not found"
    ));
  }

  Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_folder(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(folder_id): AxumPath<i64>
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let rows = if let Some(pool) =
    &state.postgres
  {
    sqlx::query("DELETE FROM folders WHERE id = $1 AND user_id = $2")
            .bind(folder_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query("DELETE FROM folders WHERE id = ?1 AND user_id = ?2")
            .bind(folder_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
  };

  if rows == 0 {
    return Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "folder not found"
    ));
  }

  Ok(StatusCode::NO_CONTENT)
}

pub async fn list_folder_feeds(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(folder_id): AxumPath<i64>
) -> Result<
  Json<Vec<FolderFeedRow>>,
  ServerError
> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  if let Some(pool) = &state.postgres {
    let rows = sqlx::query_as::<_, FolderFeedRow>(
            "SELECT ff.feed_id FROM folder_feeds ff \n             JOIN folders f ON f.id = ff.folder_id \n             WHERE ff.folder_id = $1 AND f.user_id = $2\n             ORDER BY ff.feed_id",
        )
        .bind(folder_id)
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

  let rows =
    sqlx::query_as::<_, FolderFeedRow>(
      "SELECT ff.feed_id FROM \
       folder_feeds ff \n         \
       JOIN folders f ON f.id = \
       ff.folder_id \n         WHERE \
       ff.folder_id = ?1 AND \
       f.user_id = ?2\n         ORDER \
       BY ff.feed_id"
    )
    .bind(folder_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e.to_string(),
    )
    })?;

  Ok(Json(rows))
}

pub async fn add_folder_feed(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(folder_id): AxumPath<i64>,
  Json(payload): Json<
    FolderFeedRequest
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

  let rows = if let Some(pool) =
    &state.postgres
  {
    sqlx::query(
            "INSERT INTO folder_feeds (folder_id, feed_id, created_at) \n             SELECT $1, $2, NOW() \n             WHERE EXISTS (SELECT 1 FROM folders WHERE id = $1 AND user_id = $3)",
        )
        .bind(folder_id)
        .bind(feed_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| map_db_error(e, "folder feed create failed"))?
        .rows_affected()
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query(
      "INSERT INTO folder_feeds \
       (folder_id, feed_id, \
       created_at) \n             \
       SELECT ?1, ?2, datetime('now') \
       \n             WHERE EXISTS \
       (SELECT 1 FROM folders WHERE \
       id = ?1 AND user_id = ?3)"
    )
    .bind(folder_id)
    .bind(feed_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "folder feed create failed"
      )
    })?
    .rows_affected()
  };

  if rows == 0 {
    return Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "folder not found"
    ));
  }

  Ok(StatusCode::CREATED)
}

pub async fn delete_folder_feed(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath((folder_id, feed_id)): AxumPath<(i64, String)>
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let rows = if let Some(pool) =
    &state.postgres
  {
    sqlx::query(
            "DELETE FROM folder_feeds ff \n             USING folders f \n             WHERE ff.folder_id = f.id AND ff.folder_id = $1 AND ff.feed_id = $2 AND f.user_id = $3",
        )
        .bind(folder_id)
        .bind(&feed_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .rows_affected()
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query(
            "DELETE FROM folder_feeds \n             WHERE folder_id = ?1 AND feed_id = ?2 \n               AND EXISTS (SELECT 1 FROM folders WHERE id = ?1 AND user_id = ?3)",
        )
        .bind(folder_id)
        .bind(&feed_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .rows_affected()
  };

  if rows == 0 {
    return Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "folder feed not found"
    ));
  }

  Ok(StatusCode::NO_CONTENT)
}

pub async fn list_folder_entries(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(folder_id): AxumPath<i64>,
  Query(query): Query<
    FolderEntriesQuery
  >
) -> Result<
  Json<EntryListResponse>,
  ServerError
> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let limit =
    query.limit.unwrap_or(50).min(200)
      as i64;

  let offset =
    query.offset.unwrap_or(0) as i64;

  let read_filter =
    query.read.as_deref();

  let since = query.since;

  if let Some(pool) = &state.postgres {
    let schema = state
      .fetcher_schema
      .as_deref()
      .unwrap_or("fetcher");

    let mut builder = QueryBuilder::<
            Postgres,
        >::new(
            format!(
            "SELECT fi.id, fi.feed_id, fi.title, fi.link,             CAST(EXTRACT(EPOCH FROM fi.published_at) * 1000 AS BIGINT) AS published_at_ms,             (es.read_at IS NOT NULL) AS is_read             FROM folder_feeds ff             JOIN {}.feed_items fi ON fi.feed_id = ff.feed_id             JOIN folders f ON f.id = ff.folder_id             LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = ",
            quote_ident(schema)
        )
        );

    builder.push_bind(user_id);

    builder
      .push(" WHERE ff.folder_id = ");

    builder.push_bind(folder_id);

    builder.push(" AND f.user_id = ");

    builder.push_bind(user_id);

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

    if let Some(since_id) = since {
      builder.push(" AND fi.id > ");

      builder.push_bind(since_id);
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
        since
      }
    ));
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let mut builder =
    QueryBuilder::<Sqlite>::new(
      "SELECT fi.id, fi.feed_id, \
       fi.title, fi.link,         \
       fi.published_at_ms,         \
       (es.read_at IS NOT NULL) AS \
       is_read         FROM \
       folder_feeds ff         JOIN \
       feed_items fi ON fi.feed_id = \
       ff.feed_id         JOIN \
       folders f ON f.id = \
       ff.folder_id         LEFT JOIN \
       entry_states es ON es.item_id \
       = fi.id AND es.user_id = "
    );

  builder.push_bind(user_id);

  builder
    .push(" WHERE ff.folder_id = ");

  builder.push_bind(folder_id);

  builder.push(" AND f.user_id = ");

  builder.push_bind(user_id);

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

  if let Some(since_id) = since {
    builder.push(" AND fi.id > ");

    builder.push_bind(since_id);
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
    since
  }))
}

pub async fn folder_feed_unread_counts(
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(folder_id): AxumPath<i64>
) -> Result<
  Json<Vec<FolderFeedUnreadCount>>,
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
       unread_count 
             FROM folder_feeds ff 
             JOIN folders f ON f.id = \
       ff.folder_id 
             JOIN {}.feed_items fi ON \
       fi.feed_id = ff.feed_id 
             LEFT JOIN entry_states es \
       ON es.item_id = fi.id AND \
       es.user_id = $1 
             WHERE ff.folder_id = $2 \
       AND f.user_id = $1 AND \
       es.read_at IS NULL 
             GROUP BY fi.feed_id 
             ORDER BY fi.feed_id",
      quote_ident(schema)
    );

    let rows = sqlx::query_as::<_, FolderFeedUnreadCount>(&query)
            .bind(user_id)
            .bind(folder_id)
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
    FolderFeedUnreadCount
  >(
    "SELECT fi.feed_id, COUNT(*) AS \
     unread_count 
         FROM folder_feeds ff 
         JOIN folders f ON f.id = \
     ff.folder_id 
         JOIN feed_items fi ON \
     fi.feed_id = ff.feed_id 
         LEFT JOIN entry_states es ON \
     es.item_id = fi.id AND \
     es.user_id = ?1 
         WHERE ff.folder_id = ?2 AND \
     f.user_id = ?1 AND es.read_at IS \
     NULL 
         GROUP BY fi.feed_id 
         ORDER BY fi.feed_id"
  )
  .bind(user_id)
  .bind(folder_id)
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

pub async fn folder_unread_counts(
  State(state): State<AppState>,
  headers: HeaderMap
) -> Result<
  Json<Vec<FolderUnreadCount>>,
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
            "SELECT f.id AS folder_id, COUNT(*)::BIGINT AS unread_count \n             FROM folders f \n             JOIN folder_feeds ff ON ff.folder_id = f.id \n             JOIN {}.feed_items fi ON fi.feed_id = ff.feed_id \n             LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = $1 \n             WHERE f.user_id = $1 AND es.read_at IS NULL \n             GROUP BY f.id \n             ORDER BY f.id",
            quote_ident(schema)
        );

    let rows = sqlx::query_as::<_, FolderUnreadCount>(&query)
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

  let rows = sqlx::query_as::<_, FolderUnreadCount>(
        "SELECT f.id AS folder_id, COUNT(*) AS unread_count \n         FROM folders f \n         JOIN folder_feeds ff ON ff.folder_id = f.id \n         JOIN feed_items fi ON fi.feed_id = ff.feed_id \n         LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = ?1 \n         WHERE f.user_id = ?1 AND es.read_at IS NULL \n         GROUP BY f.id \n         ORDER BY f.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

  Ok(Json(rows))
}
