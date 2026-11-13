//! Database-backed login-attempt throttle. Counters are shared between
//! processes and survive service restarts. Keys are hashes of normalized
//! usernames; never store user-supplied passwords or raw attempted names here.
//!
//! The first five attempts in each 15-minute fixed window may be evaluated;
//! the sixth and subsequent attempts receive HTTP 429. A successful login
//! clears the counter. A deliberate trade-off is that an attacker who knows a
//! username can temporarily block that account. Host/network throttling is a
//! separate deployment protection and is not replaced by this mechanism.

use axum::http::StatusCode;

use crate::app_state::AppState;
use crate::auth::hash_token;
use crate::errors::ServerError;

pub const MAX_ATTEMPTS: i64 = 5;
pub const WINDOW_SECONDS: i64 = 15 * 60;
const RETENTION_SECONDS: i64 = 24 * 60 * 60;

fn username_key(username: &str) -> String {
  hash_token(&username.trim().to_ascii_lowercase())
}

fn failed(err: sqlx::Error) -> ServerError {
  ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("login throttle database error: {err}"))
}

/// Reserve an attempt *before* looking up an account. The upsert is atomic
/// in PostgreSQL and SQLite, including when multiple logins race.
pub async fn reserve_attempt(state: &AppState, username: &str) -> Result<(), ServerError> {
  let key = username_key(username);
  let count = if let Some(pool) = &state.postgres {
    sqlx::query("DELETE FROM login_rate_limits WHERE window_started_at < EXTRACT(EPOCH FROM NOW())::BIGINT - $1")
      .bind(RETENTION_SECONDS)
      .execute(pool)
      .await
      .map_err(failed)?;
    sqlx::query_scalar::<_, i64>(
      "INSERT INTO login_rate_limits (username_key, attempt_count, window_started_at) \
       VALUES ($1, 1, EXTRACT(EPOCH FROM NOW())::BIGINT) \
       ON CONFLICT (username_key) DO UPDATE SET \
       attempt_count = CASE WHEN login_rate_limits.window_started_at <= EXTRACT(EPOCH FROM NOW())::BIGINT - $2 \
         THEN 1 ELSE login_rate_limits.attempt_count + 1 END, \
       window_started_at = CASE WHEN login_rate_limits.window_started_at <= EXTRACT(EPOCH FROM NOW())::BIGINT - $2 \
         THEN EXTRACT(EPOCH FROM NOW())::BIGINT ELSE login_rate_limits.window_started_at END \
       RETURNING attempt_count"
    )
      .bind(&key)
      .bind(WINDOW_SECONDS)
      .fetch_one(pool)
      .await
      .map_err(failed)?
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing")
    })?;
    sqlx::query("DELETE FROM login_rate_limits WHERE window_started_at < unixepoch() - ?1")
      .bind(RETENTION_SECONDS)
      .execute(pool)
      .await
      .map_err(failed)?;
    sqlx::query_scalar::<_, i64>(
      "INSERT INTO login_rate_limits (username_key, attempt_count, window_started_at) \
       VALUES (?1, 1, unixepoch()) \
       ON CONFLICT (username_key) DO UPDATE SET \
       attempt_count = CASE WHEN login_rate_limits.window_started_at <= unixepoch() - ?2 \
         THEN 1 ELSE login_rate_limits.attempt_count + 1 END, \
       window_started_at = CASE WHEN login_rate_limits.window_started_at <= unixepoch() - ?2 \
         THEN unixepoch() ELSE login_rate_limits.window_started_at END \
       RETURNING attempt_count"
    )
      .bind(&key)
      .bind(WINDOW_SECONDS)
      .fetch_one(pool)
      .await
      .map_err(failed)?
  };

  if count > MAX_ATTEMPTS {
    return Err(ServerError::new(
      StatusCode::TOO_MANY_REQUESTS,
      "too many login attempts; try again later",
    ));
  }
  Ok(())
}

/// Only call after successfully issuing a token.
pub async fn clear_attempts(state: &AppState, username: &str) -> Result<(), ServerError> {
  let key = username_key(username);
  if let Some(pool) = &state.postgres {
    sqlx::query("DELETE FROM login_rate_limits WHERE username_key = $1")
      .bind(key).execute(pool).await.map_err(failed)?;
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing")
    })?;
    sqlx::query("DELETE FROM login_rate_limits WHERE username_key = ?1")
      .bind(key).execute(pool).await.map_err(failed)?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use axum::response::IntoResponse;
  use sqlx::sqlite::SqlitePoolOptions;

  async fn fixture() -> AppState {
    let pool = SqlitePoolOptions::new().max_connections(1)
      .connect("sqlite::memory:").await.expect("sqlite");
    sqlx::query(
      "CREATE TABLE login_rate_limits (username_key TEXT PRIMARY KEY, \
       attempt_count INTEGER NOT NULL, window_started_at INTEGER NOT NULL)"
    ).execute(&pool).await.expect("throttle schema");
    AppState {sqlite: Some(pool), postgres: None, fetcher_schema: None, token_ttl_seconds: 3600}
  }

  #[tokio::test]
  async fn concurrent_reservations_are_atomic() {
    let state = fixture().await;
    let mut handles = Vec::new();
    for _ in 0..16 {
      let cloned = state.clone();
      handles.push(tokio::spawn(async move {
        reserve_attempt(&cloned, "Concurrent User").await
      }));
    }
    let mut admitted = 0;
    let mut rejected = 0;
    for handle in handles {
      match handle.await.expect("join") {
        Ok(()) => admitted += 1,
        Err(err) => {
          assert_eq!(err.into_response().status(), StatusCode::TOO_MANY_REQUESTS);
          rejected += 1;
        }
      }
    }
    assert_eq!(admitted, MAX_ATTEMPTS);
    assert_eq!(rejected, 16 - MAX_ATTEMPTS);
  }

  #[tokio::test]
  async fn expiry_and_success_clear_the_window() {
    let state = fixture().await;
    for _ in 0..MAX_ATTEMPTS {
      reserve_attempt(&state, "Unknown").await.expect("within limit");
    }
    assert_eq!(
      reserve_attempt(&state, "unknown").await.expect_err("blocked").into_response().status(),
      StatusCode::TOO_MANY_REQUESTS
    );
    let pool = state.sqlite.as_ref().expect("sqlite");
    sqlx::query("UPDATE login_rate_limits SET window_started_at = unixepoch() - ?1")
      .bind(WINDOW_SECONDS + 1).execute(pool).await.expect("old window");
    reserve_attempt(&state, "UNKNOWN").await.expect("expired window resets");
    let count: i64 = sqlx::query_scalar("SELECT attempt_count FROM login_rate_limits")
      .fetch_one(pool).await.expect("attempts");
    assert_eq!(count, 1);
    clear_attempts(&state, "unknown").await.expect("successful auth clears");
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM login_rate_limits")
      .fetch_one(pool).await.expect("no rows");
    assert_eq!(left, 0);
  }
}
