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

  let password =
    payload.password.trim();

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

    sqlx::query(
      "INSERT INTO users (username, \
       password_hash, created_at) \
       VALUES (?1, ?2, \
       datetime('now'))"
    )
    .bind(username)
    .bind(&password_hash)
    .execute(pool)
    .await
    .map_err(|e| {
      map_db_error(
        e,
        "user create failed"
      )
    })?;

    sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
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
    payload.current_password.trim();

  let new_password =
    payload.new_password.trim();

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

pub async fn delete_user(
  State(state): State<AppState>,
  headers: axum::http::HeaderMap
) -> Result<StatusCode, ServerError> {
  let user_id =
    auth_user_id(&state, &headers)
      .await?;

  let rows = if let Some(pool) =
    &state.postgres
  {
    sqlx::query("DELETE FROM users WHERE id = $1")
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

    sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .rows_affected()
  };

  if rows == 0 {
    return Err(ServerError::new(
      StatusCode::NOT_FOUND,
      "user not found"
    ));
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
}
