use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;

use crate::app_state::AppState;

/// Liveness never depends on a DB
/// query.
pub async fn health() -> &'static str {
  "ok"
}

/// Readiness requires a responsive,
/// configured database. Do not disclose
/// connection or credential details to
/// callers.
pub async fn ready(
  State(state): State<AppState>
) -> (StatusCode, &'static str) {
  let available = if let Some(pool) =
    &state.postgres
  {
    tokio::time::timeout(
      Duration::from_secs(2),
      sqlx::query("SELECT 1")
        .execute(pool)
    )
    .await
    .is_ok_and(|result| result.is_ok())
  } else if let Some(pool) =
    &state.sqlite
  {
    tokio::time::timeout(
      Duration::from_secs(2),
      sqlx::query("SELECT 1")
        .execute(pool)
    )
    .await
    .is_ok_and(|result| result.is_ok())
  } else {
    false
  };

  if available {
    (StatusCode::OK, "ready")
  } else {
    (
      StatusCode::SERVICE_UNAVAILABLE,
      "not ready"
    )
  }
}

#[cfg(test)]
mod readiness_tests {
  use super::*;

  fn state(
    pool: Option<sqlx::SqlitePool>
  ) -> AppState {
    AppState {
      sqlite:            pool,
      postgres:          None,
      fetcher_schema:    None,
      token_ttl_seconds: 60
    }
  }

  #[tokio::test]
  async fn readiness_requires_database_pool()
   {
    let (status, text) =
      ready(State(state(None))).await;
    assert_eq!(
      status,
      StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(text, "not ready");
    assert_eq!(health().await, "ok");
  }

  #[tokio::test]
  async fn readiness_reports_sqlite_connectivity_and_shutdown()
   {
    let pool =
      sqlx::SqlitePool::connect(
        "sqlite::memory:"
      )
      .await
      .expect("isolated SQLite pool");
    let state =
      state(Some(pool.clone()));
    let (status, text) =
      ready(State(state.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(text, "ready");

    pool.close().await;
    let (status, text) =
      ready(State(state)).await;
    assert_eq!(
      status,
      StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(text, "not ready");
  }
}
