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

#[cfg(test)]
mod sqlite_pool_folder_tests {
  use std::time::{
    Duration,
    SystemTime,
    UNIX_EPOCH
  };

  use sqlx::sqlite::{
    SqliteConnectOptions,
    SqliteJournalMode,
    SqlitePoolOptions
  };

  use super::*;
  use crate::auth::hash_token;

  #[tokio::test]
  async fn sqlite_pooled_folder_creation_returns_each_inserted_id()
   {
    let stamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("fixture timestamp")
      .as_nanos();
    let path = std::env::temp_dir().join(
      format!(
        "pulsewire-folder-pool-{}-{stamp}.sqlite",
        std::process::id()
      )
    );
    let pool = SqlitePoolOptions::new()
      .max_connections(8)
      .connect_with(
        SqliteConnectOptions::new()
          .filename(&path)
          .create_if_missing(true)
          .journal_mode(
            SqliteJournalMode::Wal
          )
          .busy_timeout(
            Duration::from_secs(10)
          )
      )
      .await
      .expect("isolated SQLite pool");
    for sql in [
      "CREATE TABLE user_tokens (user_id INTEGER NOT NULL, token_hash TEXT NOT NULL, expires_at TEXT NOT NULL)",
      "CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL, name TEXT NOT NULL, created_at TEXT NOT NULL)"
    ] {
      sqlx::query(sql)
        .execute(&pool)
        .await
        .expect("setup disposable table");
    }
    let bearer = "synthetic-only-bearer";
    sqlx::query(
      "INSERT INTO user_tokens (user_id, token_hash, expires_at) VALUES (7, ?1, datetime('now', '+1 day'))"
    )
    .bind(hash_token(bearer))
    .execute(&pool)
    .await
    .expect("seed synthetic bearer");
    let mut headers =
      HeaderMap::new();
    headers.insert(
      "authorization",
      format!("Bearer {bearer}")
        .parse()
        .expect("bearer header")
    );
    let state = AppState {
      sqlite: Some(pool.clone()),
      postgres: None,
      fetcher_schema: None,
      token_ttl_seconds: 3600
    };
    let mut pending =
      tokio::task::JoinSet::new();
    for n in 0..32 {
      let state = state.clone();
      let headers = headers.clone();
      pending.spawn(async move {
        let name = format!(
          "fixture folder {n}"
        );
        let result = create_folder(
          State(state),
          headers,
          Json(FolderCreateRequest {
            name: name.clone()
          })
        )
        .await;
        (name, result)
      });
    }
    let mut ids =
      std::collections::HashSet::new();
    while let Some(task) =
      pending.join_next().await
    {
      let (name, result) =
        task.expect("join folder request");
      let Json(folder) = result
        .expect("concurrent folder creation");
      assert_eq!(folder.name, name);
      assert!(ids.insert(folder.id));
      let stored: String =
        sqlx::query_scalar(
          "SELECT name FROM folders WHERE id = ?1 AND user_id = 7"
        )
        .bind(folder.id)
        .fetch_one(&pool)
        .await
        .expect("folder belongs to own insert");
      assert_eq!(stored, name);
    }
    assert_eq!(ids.len(), 32);
    pool.close().await;
    let _ = std::fs::remove_file(path);
  }
}
