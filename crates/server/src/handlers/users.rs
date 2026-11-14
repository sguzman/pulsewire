use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::app_state::AppState;
use crate::auth::{
  auth_user_id,
  bearer_token,
  hash_password,
  hash_token,
  require_admin,
  verify_password
};
use crate::errors::{
  ServerError,
  map_db_error
};
use crate::models::{
  CreateUserRequest,
  PasswordChangeRequest,
  UserResponse
};

pub async fn create_user(
  State(state): State<AppState>,
  headers: axum::http::HeaderMap,
  Json(payload): Json<
    CreateUserRequest
  >
) -> Result<
  Json<UserResponse>,
  ServerError
> {
  require_admin(&state, &headers)
    .await?;

  let username =
    payload.username.trim();

  // Passwords are opaque bytes:
  // trimming silently changes the
  // credential that a user selected.
  let password =
    payload.password.as_str();

  if username.is_empty()
    || password.len() < 12
  {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "username required; password \
       must be at least 12 bytes"
    ));
  }

  let password_hash = hash_password(
    password
  )
  .map_err(|e| {
    ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e
    )
  })?;

  let user_id = if let Some(pool) =
    &state.postgres
  {
    sqlx::query_scalar::<_, i64>(
      "INSERT INTO users (username, \
       password_hash, created_at) \
       VALUES ($1, $2, NOW()) \
       RETURNING id"
    )
    .bind(username)
    .bind(&password_hash)
    .fetch_one(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "user create failed"
      )
    })?
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    // The returned id must come from
    // the INSERT itself:
    // last_insert_rowid() is
    // connection-local and a pooled
    // follow-up query can observe a
    // different connection.
    sqlx::query_scalar::<_, i64>(
      "INSERT INTO users (username, \
       password_hash, created_at) \
       VALUES (?1, ?2, \
       datetime('now')) RETURNING id"
    )
    .bind(username)
    .bind(&password_hash)
    .fetch_one(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "user create failed"
      )
    })?
  };

  Ok(Json(UserResponse {
    id:       user_id,
    username: username.to_string()
  }))
}

pub async fn change_password(
  State(state): State<AppState>,
  headers: axum::http::HeaderMap,
  Json(payload): Json<
    PasswordChangeRequest
  >
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;
  let session_hash = hash_token(
    &bearer_token(&headers)?
  );

  let current_password =
    payload.current_password.as_str();

  let new_password =
    payload.new_password.as_str();

  if current_password.is_empty()
    || new_password.len() < 12
  {
    return Err(ServerError::new(
      StatusCode::BAD_REQUEST,
      "current_password required; \
       new_password must be at least \
       12 bytes"
    ));
  }

  let password_hash = if let Some(
    pool
  ) =
    &state.postgres
  {
    sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "user not found"))?
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| ServerError::new(StatusCode::NOT_FOUND, "user not found"))?
  };

  verify_password(
    &password_hash,
    current_password
  )
  .map_err(|_| {
    ServerError::new(
      StatusCode::UNAUTHORIZED,
      "invalid credentials"
    )
  })?;

  let new_hash =
    hash_password(new_password)
      .map_err(|e| {
        ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      e,
    )
      })?;

  if let Some(pool) = &state.postgres {
    let mut tx = pool.begin().await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Verify the original password and
    // active bearer again while holding
    // the same user lock as operator
    // recovery.
    let locked_hash: Option<String> = sqlx::query_scalar(
      "SELECT password_hash FROM users WHERE id = $1 FOR UPDATE"
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if locked_hash.as_deref()
      != Some(password_hash.as_str())
    {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid credentials"
      ));
    }
    let still_active: Option<i64> = sqlx::query_scalar(
      "SELECT user_id FROM user_tokens WHERE user_id = $1 AND token_hash = $2 AND expires_at > NOW()"
    )
    .bind(user_id)
    .bind(&session_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if still_active.is_none() {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid token"
      ));
    }
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
      .bind(&new_hash).bind(user_id).execute(&mut *tx).await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM user_tokens WHERE user_id = $1")
      .bind(user_id).execute(&mut *tx).await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM user_password_resets WHERE user_id = $1")
      .bind(user_id).execute(&mut *tx).await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tx.commit().await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  } else {
    let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

    let mut tx = pool.begin().await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Acquire SQLite's write lock
    // before rechecking the
    // password hash and session; a
    // plain SELECT is too late.
    let locked_hash: Option<String> = sqlx::query_scalar(
      "UPDATE users SET password_hash = password_hash WHERE id = ?1 RETURNING password_hash"
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if locked_hash.as_deref()
      != Some(password_hash.as_str())
    {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid credentials"
      ));
    }
    let still_active: Option<i64> = sqlx::query_scalar(
      "SELECT user_id FROM user_tokens WHERE user_id = ?1 AND token_hash = ?2 AND expires_at > datetime('now')"
    )
    .bind(user_id)
    .bind(&session_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if still_active.is_none() {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid token"
      ));
    }
    sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
      .bind(&new_hash).bind(user_id).execute(&mut *tx).await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM user_tokens WHERE user_id = ?1")
      .bind(user_id).execute(&mut *tx).await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM user_password_resets WHERE user_id = ?1")
      .bind(user_id).execute(&mut *tx).await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tx.commit().await
      .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  }

  Ok(StatusCode::NO_CONTENT)
}

/// Self-service deletion. The last
/// administrator cannot delete
/// themselves, which would leave
/// the instance without an
/// administrative path.
pub async fn delete_user(
  State(state): State<AppState>,
  headers: axum::http::HeaderMap
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;
  let token_hash = hash_token(
    &bearer_token(&headers)?
  );
  if let Some(pool) = &state.postgres {
    let mut tx = pool.begin().await.map_err(|e| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
      )
    })?;
    // Serialize concurrent admin
    // deletions in a stable row-lock
    // order. Login and recovery
    // also lock their account row.
    let admins: Vec<i64> = sqlx::query_scalar(
      "SELECT id FROM users WHERE is_admin = TRUE \
       ORDER BY id FOR UPDATE"
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    let is_admin: Option<bool> = sqlx::query_scalar(
      "SELECT is_admin FROM users WHERE id = $1 FOR UPDATE"
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    let Some(is_admin) = is_admin
    else {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid token"
      ));
    };
    let active: Option<i64> = sqlx::query_scalar(
      "SELECT user_id FROM user_tokens \
       WHERE user_id = $1 AND token_hash = $2 \
       AND expires_at > NOW()"
    )
    .bind(user_id)
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    if active.is_none() {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid token"
      ));
    }
    if is_admin && admins.len() <= 1 {
      return Err(ServerError::new(
        StatusCode::CONFLICT,
        "cannot delete the last \
         administrator"
      ));
    }
    // Explicit token cleanup is
    // intentional, even where cascading
    // foreign keys are present.
    sqlx::query(
      "DELETE FROM user_tokens WHERE user_id = $1"
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    sqlx::query(
      "DELETE FROM user_password_resets WHERE user_id = $1"
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    sqlx::query("DELETE FROM users WHERE id = $1")
      .bind(user_id)
      .execute(&mut *tx)
      .await
      .map_err(|e| ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
      ))?;
    tx.commit().await.map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"
      )
    })?;
    let mut tx = pool.begin().await.map_err(|e| {
      ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
      )
    })?;
    // Acquire SQLite's writer lock
    // before checking the admin count
    // or session validity.
    let is_admin: Option<i64> = sqlx::query_scalar(
      "UPDATE users SET is_admin = is_admin \
       WHERE id = ?1 RETURNING is_admin"
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    let Some(is_admin) = is_admin
    else {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid token"
      ));
    };
    let active: Option<i64> = sqlx::query_scalar(
      "SELECT user_id FROM user_tokens \
       WHERE user_id = ?1 AND token_hash = ?2 \
       AND expires_at > datetime('now')"
    )
    .bind(user_id)
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    if active.is_none() {
      return Err(ServerError::new(
        StatusCode::UNAUTHORIZED,
        "invalid token"
      ));
    }
    if is_admin != 0 {
      let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE is_admin != 0"
      )
      .fetch_one(&mut *tx)
      .await
      .map_err(|e| ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
      ))?;
      if count <= 1 {
        return Err(ServerError::new(
          StatusCode::CONFLICT,
          "cannot delete the last \
           administrator"
        ));
      }
    }
    sqlx::query(
      "DELETE FROM user_tokens WHERE user_id = ?1"
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    sqlx::query(
      "DELETE FROM user_password_resets WHERE user_id = ?1"
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
    sqlx::query("DELETE FROM users WHERE id = ?1")
      .bind(user_id)
      .execute(&mut *tx)
      .await
      .map_err(|e| ServerError::new(
        StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
      ))?;
    tx.commit().await.map_err(|e| ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR, e.to_string()
    ))?;
  }

  Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod security_tests {
  use axum::http::HeaderMap;
  use axum::response::IntoResponse;
  use sqlx::sqlite::SqlitePoolOptions;

  use super::*;
  use crate::handlers::auth;
  use crate::models::LoginRequest;

  async fn setup() -> AppState {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("sqlite memory");
    sqlx::query(
      "CREATE TABLE users (id INTEGER \
       PRIMARY KEY AUTOINCREMENT, \
       username TEXT NOT NULL UNIQUE, \
       password_hash TEXT NOT NULL, \
       is_admin INTEGER NOT NULL \
       DEFAULT 0, created_at TEXT NOT \
       NULL)"
    )
    .execute(&pool)
    .await
    .expect("users table");
    sqlx::query(
      "CREATE TABLE user_tokens (id \
       INTEGER PRIMARY KEY \
       AUTOINCREMENT, user_id INTEGER \
       NOT NULL, token_hash TEXT NOT \
       NULL, expires_at TEXT NOT \
       NULL, created_at TEXT NOT NULL)"
    )
    .execute(&pool)
    .await
    .expect("tokens table");
    sqlx::query(
      "CREATE TABLE \
       user_password_resets (id \
       INTEGER PRIMARY KEY, user_id \
       INTEGER NOT NULL, token_hash \
       TEXT NOT NULL, expires_at TEXT \
       NOT NULL, created_at TEXT NOT \
       NULL)"
    )
    .execute(&pool)
    .await
    .expect("reset tokens table");
    sqlx::query(
      "CREATE TABLE login_rate_limits \
       (username_key TEXT PRIMARY \
       KEY, attempt_count INTEGER NOT \
       NULL, window_started_at \
       INTEGER NOT NULL)"
    )
    .execute(&pool)
    .await
    .expect("login throttle table");
    AppState {
      sqlite:            Some(pool),
      postgres:          None,
      fetcher_schema:    None,
      token_ttl_seconds: 3600
    }
  }

  async fn login_headers(
    state: &AppState,
    name: &str,
    password: &str
  ) -> HeaderMap {
    let Json(token) = auth::login(
      State(state.clone()),
      Json(LoginRequest {
        username: name.to_string(),
        password: password.to_string()
      })
    )
    .await
    .expect("login");
    let mut headers = HeaderMap::new();
    headers.insert(
      "authorization",
      format!("Bearer {}", token.token)
        .parse()
        .expect("token header")
    );
    headers
  }

  #[tokio::test]
  async fn only_authenticated_administrators_can_create_users()
   {
    let state = setup().await;
    let request = CreateUserRequest {
      username: "second".into(),
      password:
        "sufficiently-long-passphrase"
          .into()
    };
    let err = create_user(
      State(state.clone()),
      HeaderMap::new(),
      Json(request)
    )
    .await
    .expect_err(
      "anonymous registration must \
       fail"
    );
    assert_eq!(
      err.into_response().status(),
      StatusCode::UNAUTHORIZED
    );
    crate::db::bootstrap_admin(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await
    .expect("bootstrap owner");
    let admin_headers = login_headers(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await;

    let Json(created) = create_user(
      State(state.clone()),
      admin_headers,
      Json(CreateUserRequest {
        username: "reader".into(),
        password:
          "reader-long-passphrase"
            .into()
      })
    )
    .await
    .expect(
      "administrator creates reader"
    );
    assert_eq!(
      created.username,
      "reader"
    );

    let reader_headers = login_headers(
      &state,
      "reader",
      "reader-long-passphrase"
    )
    .await;
    let err = create_user(
      State(state.clone()),
      reader_headers,
      Json(CreateUserRequest {
        username: "third".into(),
        password:
          "other-long-passphrase".into()
      })
    )
    .await
    .expect_err(
      "non-admin registration must \
       fail"
    );
    assert_eq!(
      err.into_response().status(),
      StatusCode::FORBIDDEN
    );

    let pool = state
      .sqlite
      .as_ref()
      .expect("sqlite");
    let count: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM users"
      )
      .fetch_one(pool)
      .await
      .expect("count");
    assert_eq!(count, 2);
    let reader_admin: i64 =
      sqlx::query_scalar(
        "SELECT is_admin FROM users \
         WHERE username = 'reader'"
      )
      .fetch_one(pool)
      .await
      .expect("reader role");
    assert_eq!(reader_admin, 0);
  }

  #[tokio::test]
  async fn password_change_revokes_all_existing_sessions()
   {
    let state = setup().await;
    crate::db::bootstrap_admin(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await
    .expect("bootstrap owner");
    let current_token = login_headers(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await;
    let another_token = login_headers(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await;

    change_password(
      State(state.clone()),
      current_token.clone(),
      Json(PasswordChangeRequest {
        current_password: "owner-long-passphrase".into(),
        new_password: "brand-new-long-passphrase".into()
      })
    ).await.expect("change password");

    for token in
      [current_token, another_token]
    {
      let err =
        crate::auth::auth_user_id(
          &state, &token
        )
        .await
        .expect_err(
          "old token must not \
           authorize"
        );
      assert_eq!(
        err.into_response().status(),
        StatusCode::UNAUTHORIZED
      );
    }

    let stale_login = auth::login(
      State(state.clone()),
      Json(LoginRequest {
        username: "owner".into(),
        password:
          "owner-long-passphrase".into()
      })
    )
    .await
    .expect_err(
      "old password must not work"
    );
    assert_eq!(
      stale_login
        .into_response()
        .status(),
      StatusCode::UNAUTHORIZED
    );
    let _fresh_token = login_headers(
      &state,
      "owner",
      "brand-new-long-passphrase"
    )
    .await;
  }

  #[tokio::test]
  async fn bootstrap_cannot_create_second_admin_on_nonempty_database()
   {
    let state = setup().await;
    crate::db::bootstrap_admin(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await
    .expect("bootstrap");
    assert!(
      crate::db::bootstrap_admin(
        &state,
        "outsider",
        "outsider-long-passphrase"
      )
      .await
      .is_err()
    );
    assert!(
      crate::db::bootstrap_admin(
        &state,
        "owner",
        "owner-long-passphrase"
      )
      .await
      .is_err()
    );
  }

  #[tokio::test]
  async fn login_throttling_rejects_unknown_and_known_names_uniformly()
   {
    let state = setup().await;
    crate::db::bootstrap_admin(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await
    .expect("bootstrap");
    for username in
      ["nonexistent", "owner"]
    {
      for _ in 0..crate::login_throttle::MAX_ATTEMPTS {
        let err = auth::login(State(state.clone()), Json(LoginRequest {
          username: username.into(), password: "wrong-passphrase".into()
        })).await.expect_err("wrong credentials must fail");
        assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
      }
      let err = auth::login(
        State(state.clone()),
        Json(LoginRequest {
          username: username.into(),
          password: "wrong-passphrase"
            .into()
        })
      )
      .await
      .expect_err(
        "sixth attempt blocked"
      );
      let response =
        err.into_response();
      assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS
      );
      assert_eq!(response.headers().get(axum::http::header::RETRY_AFTER).expect("retry header"), "900");
    }
    let locked = auth::login(
      State(state.clone()),
      Json(LoginRequest {
        username: "owner".into(),
        password:
          "owner-long-passphrase".into()
      })
    )
    .await
    .expect_err(
      "correct password also respects \
       the window"
    );
    assert_eq!(
      locked.into_response().status(),
      StatusCode::TOO_MANY_REQUESTS
    );
    let pool = state
      .sqlite
      .as_ref()
      .expect("sqlite");
    sqlx::query(
      "UPDATE login_rate_limits SET \
       window_started_at = \
       unixepoch() - 901"
    )
    .execute(pool)
    .await
    .expect("expire buckets");
    let _fresh = login_headers(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await;
    let key =
      crate::auth::hash_token("owner");
    let existing: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM \
         login_rate_limits WHERE \
         username_key = ?1"
      )
      .bind(key)
      .fetch_one(pool)
      .await
      .expect("cleared");
    assert_eq!(existing, 0);
  }

  #[tokio::test]
  async fn deletion_protects_last_admin_and_revokes_reader()
   {
    let state = setup().await;
    crate::db::bootstrap_admin(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await
    .expect("bootstrap");
    let admin = login_headers(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await;
    let result = delete_user(
      State(state.clone()),
      admin.clone()
    )
    .await
    .expect_err(
      "last administrator deletion \
       rejected"
    );
    assert_eq!(
      result.into_response().status(),
      StatusCode::CONFLICT
    );

    let password = "  reader-passphrase-with-spaces  ";
    let Json(reader) = create_user(
      State(state.clone()),
      admin.clone(),
      Json(CreateUserRequest {
        username: "reader".into(),
        password: password.into()
      })
    )
    .await
    .expect("create reader");
    let pool = state
      .sqlite
      .as_ref()
      .expect("sqlite");
    let stored_id: i64 =
      sqlx::query_scalar(
        "SELECT id FROM users WHERE \
         username = 'reader'"
      )
      .fetch_one(pool)
      .await
      .expect("stored reader id");
    assert_eq!(reader.id, stored_id);

    let err = auth::login(
      State(state.clone()),
      Json(LoginRequest {
        username: "reader".into(),
        password: password
          .trim()
          .into()
      })
    )
    .await
    .expect_err(
      "password whitespace preserved"
    );
    assert_eq!(
      err.into_response().status(),
      StatusCode::UNAUTHORIZED
    );
    let reader_headers = login_headers(
      &state, "reader", password
    )
    .await;
    sqlx::query(
      "INSERT INTO \
       user_password_resets (user_id, \
       token_hash, expires_at, \
       created_at) VALUES (?1, \
       'old-reset', datetime('now', \
       '+1 hour'), datetime('now'))"
    )
    .bind(stored_id)
    .execute(pool)
    .await
    .expect("reset fixture");

    delete_user(
      State(state.clone()),
      reader_headers.clone()
    )
    .await
    .expect("reader self-deletion");
    assert!(
      crate::auth::auth_user_id(
        &state,
        &reader_headers
      )
      .await
      .is_err()
    );
    let users: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM users"
      )
      .fetch_one(pool)
      .await
      .expect("users");
    let tokens: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM \
         user_tokens WHERE user_id = \
         ?1"
      )
      .bind(stored_id)
      .fetch_one(pool)
      .await
      .expect("tokens");
    let resets: i64 =
      sqlx::query_scalar(
        "SELECT COUNT(*) FROM \
         user_password_resets WHERE \
         user_id = ?1"
      )
      .bind(stored_id)
      .fetch_one(pool)
      .await
      .expect("resets");
    assert_eq!(users, 1);
    assert_eq!(tokens, 0);
    assert_eq!(resets, 0);
    assert!(
      crate::auth::auth_user_id(
        &state, &admin
      )
      .await
      .is_ok()
    );
  }

  #[tokio::test]
  async fn deleting_one_of_two_administrators_preserves_the_other()
   {
    let state = setup().await;
    crate::db::bootstrap_admin(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await
    .expect("bootstrap");
    let owner = login_headers(
      &state,
      "owner",
      "owner-long-passphrase"
    )
    .await;
    create_user(
      State(state.clone()),
      owner.clone(),
      Json(CreateUserRequest {
        username: "second_admin".into(),
        password:
          "second-admin-passphrase"
            .into()
      })
    )
    .await
    .expect("second account");
    let pool = state
      .sqlite
      .as_ref()
      .expect("sqlite");
    sqlx::query(
      "UPDATE users SET is_admin = 1 \
       WHERE username = 'second_admin'"
    )
    .execute(pool)
    .await
    .expect("promote fixture admin");
    let other = login_headers(
      &state,
      "second_admin",
      "second-admin-passphrase"
    )
    .await;
    delete_user(
      State(state.clone()),
      owner.clone()
    )
    .await
    .expect(
      "one of two admins may delete \
       self"
    );
    assert!(
      crate::auth::auth_user_id(
        &state, &owner
      )
      .await
      .is_err()
    );
    assert!(
      crate::auth::require_admin(
        &state, &other
      )
      .await
      .is_ok()
    );
    let err = delete_user(
      State(state.clone()),
      other
    )
    .await
    .expect_err(
      "last remaining admin stays"
    );
    assert_eq!(
      err.into_response().status(),
      StatusCode::CONFLICT
    );
  }
}
