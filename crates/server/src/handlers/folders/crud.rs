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
  FolderCreateRequest,
  FolderRow,
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
    .map_err(|e| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        e.to_string(),
      )
    })?;

    return Ok(Json(rows));
  }

  let pool = state
    .sqlite
    .as_ref()
    .ok_or_else(|| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      "database pool missing",
    )
    })?;

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
    .ok_or_else(|| {
      ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      "database pool missing",
    )
    })?;

  // SQLite last_insert_rowid() is scoped
  // to an individual connection, not a
  // connection pool. Use one RETURNING
  // statement so the ID always belongs
  // to this insert.
  let row = sqlx::query_as::<_, FolderRow>(
    "INSERT INTO folders (user_id, \
     name, created_at) VALUES (?1, \
     ?2, datetime('now')) \
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
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database pool missing",
      )
    })?;

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
    sqlx::query(
      "DELETE FROM folders WHERE id = $1 AND user_id = $2",
    )
    .bind(folder_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        e.to_string(),
      )
    })?
    .rows_affected()
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database pool missing",
      )
    })?;

    sqlx::query(
      "DELETE FROM folders WHERE id = ?1 AND user_id = ?2",
    )
    .bind(folder_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        e.to_string(),
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
