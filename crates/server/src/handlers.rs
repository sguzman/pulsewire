use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json,
    Router,
};

use crate::app_state::AppState;
use crate::auth::{
    auth_user_id, bearer_token, generate_token, hash_password, hash_token, verify_password,
};
use crate::db::quote_ident;
use crate::errors::{map_db_error, ServerError};
use crate::models::{
    CreateUserRequest, EntryBatchRequest, EntryListQuery, EntrySummary, FeedSummary, LoginRequest,
    SubscriptionRequest, SubscriptionRow,
    TokenResponse, UserResponse,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/feeds", get(list_feeds))
        .route("/v1/users", post(create_user))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/entries", get(list_entries))
        .route("/v1/entries/read", post(mark_entries_read))
        .route("/v1/entries/read", delete(mark_entries_unread))
        .route("/v1/entries/:item_id/read", get(read_state))
        .route("/v1/entries/:item_id/read", post(mark_read))
        .route("/v1/entries/:item_id/read", delete(mark_unread))
        .route("/v1/subscriptions", get(list_subscriptions))
        .route("/v1/subscriptions", post(create_subscription))
        .route("/v1/subscriptions/:feed_id", delete(delete_subscription))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn list_feeds(State(state): State<AppState>) -> Result<Json<Vec<FeedSummary>>, ServerError> {
    if let Some(pool) = &state.postgres {
        let schema = state.fetcher_schema.as_deref().unwrap_or("fetcher");
        let query = format!(
            "SELECT id, url, domain, category, base_poll_seconds FROM {}.feeds ORDER BY id",
            quote_ident(schema)
        );
        let rows = sqlx::query_as::<_, FeedSummary>(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                ServerError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("feeds query failed: {e}"),
                )
            })?;
        return Ok(Json(rows));
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let rows = sqlx::query_as::<_, FeedSummary>(
        "SELECT id, url, domain, category, base_poll_seconds FROM feeds ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        ServerError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("feeds query failed: {e}"),
        )
    })?;
    Ok(Json(rows))
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, ServerError> {
    let username = payload.username.trim();
    let password = payload.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err(ServerError::new(
            StatusCode::BAD_REQUEST,
            "username and password required",
        ));
    }

    let password_hash =
        hash_password(password).map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let user_id = if let Some(pool) = &state.postgres {
        let row = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (username, password_hash, created_at) VALUES ($1, $2, NOW()) RETURNING id",
        )
        .bind(username)
        .bind(&password_hash)
        .fetch_one(pool)
        .await
        .map_err(|e| map_db_error(e, "user create failed"))?;
        row
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query(
            "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, datetime('now'))",
        )
        .bind(username)
        .bind(&password_hash)
        .execute(pool)
        .await
        .map_err(|e| map_db_error(e, "user create failed"))?;
        sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(UserResponse {
        id: user_id,
        username: username.to_string(),
    }))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ServerError> {
    let username = payload.username.trim();
    let password = payload.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err(ServerError::new(
            StatusCode::BAD_REQUEST,
            "username and password required",
        ));
    }

    let (user_id, password_hash) = if let Some(pool) = &state.postgres {
        sqlx::query_as::<_, (i64, String)>(
            "SELECT id, password_hash FROM users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query_as::<_, (i64, String)>(
            "SELECT id, password_hash FROM users WHERE username = ?1",
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?
    };

    verify_password(&password_hash, password)
        .map_err(|_| ServerError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?;

    let token = generate_token();
    let token_hash = hash_token(&token);
    let ttl = state.token_ttl_seconds as i64;

    if let Some(pool) = &state.postgres {
        sqlx::query(
            "INSERT INTO user_tokens (user_id, token_hash, expires_at, created_at) VALUES ($1, $2, NOW() + ($3 || ' seconds')::interval, NOW())",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(ttl)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query(
            "INSERT INTO user_tokens (user_id, token_hash, expires_at, created_at) VALUES (?1, ?2, datetime('now', '+' || ?3 || ' seconds'), datetime('now'))",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(ttl)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(TokenResponse {
        token,
        token_type: "bearer".to_string(),
        expires_in: state.token_ttl_seconds,
    }))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ServerError> {
    let token = bearer_token(&headers)?;
    let token_hash = hash_token(&token);

    if let Some(pool) = &state.postgres {
        sqlx::query("DELETE FROM user_tokens WHERE token_hash = $1")
            .bind(&token_hash)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query("DELETE FROM user_tokens WHERE token_hash = ?1")
            .bind(&token_hash)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn read_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(item_id): AxumPath<i64>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;

    let exists = if let Some(pool) = &state.postgres {
        sqlx::query_scalar::<_, i64>(
            "SELECT 1::BIGINT FROM entry_states WHERE user_id = $1 AND item_id = $2 AND read_at IS NOT NULL",
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some()
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM entry_states WHERE user_id = ?1 AND item_id = ?2 AND read_at IS NOT NULL",
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some()
    };

    if exists {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ServerError::new(StatusCode::NOT_FOUND, "unread"))
    }
}

async fn mark_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(item_id): AxumPath<i64>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;

    if let Some(pool) = &state.postgres {
        sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at) VALUES ($1, $2, NOW()) \
            ON CONFLICT (user_id, item_id) DO UPDATE SET read_at = EXCLUDED.read_at",
        )
        .bind(user_id)
        .bind(item_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at) VALUES (?1, ?2, datetime('now')) \
            ON CONFLICT(user_id, item_id) DO UPDATE SET read_at = excluded.read_at",
        )
        .bind(user_id)
        .bind(item_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn mark_unread(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(item_id): AxumPath<i64>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;

    if let Some(pool) = &state.postgres {
        sqlx::query("DELETE FROM entry_states WHERE user_id = $1 AND item_id = $2")
            .bind(user_id)
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        let pool = state
            .sqlite
            .as_ref()
            .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
        sqlx::query("DELETE FROM entry_states WHERE user_id = ?1 AND item_id = ?2")
            .bind(user_id)
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EntryListQuery>,
) -> Result<Json<Vec<EntrySummary>>, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let read_filter = query.read.as_deref();

    if let Some(pool) = &state.postgres {
        let schema = state.fetcher_schema.as_deref().unwrap_or("fetcher");
        let condition = match read_filter {
            Some("read") => "AND es.read_at IS NOT NULL",
            Some("unread") => "AND es.read_at IS NULL",
            Some("all") => "",
            Some(other) => {
                return Err(ServerError::new(
                    StatusCode::BAD_REQUEST,
                    format!("invalid read filter: {other}"),
                ));
            }
            None => "",
        };
        let query = format!(
            "SELECT fi.id, fi.feed_id, fi.title, fi.link,             CAST(EXTRACT(EPOCH FROM fi.published_at) * 1000 AS BIGINT) AS published_at_ms,             (es.read_at IS NOT NULL) AS is_read             FROM {}.feed_items fi             LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = $1             WHERE 1=1 {condition}             ORDER BY fi.id DESC             LIMIT $2 OFFSET $3",
            quote_ident(schema)
        );
        let rows = sqlx::query_as::<_, EntrySummary>(&query)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(rows));
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let condition = match read_filter {
        Some("read") => "AND es.read_at IS NOT NULL",
        Some("unread") => "AND es.read_at IS NULL",
        Some("all") => "",
        Some(other) => {
            return Err(ServerError::new(
                StatusCode::BAD_REQUEST,
                format!("invalid read filter: {other}"),
            ));
        }
        None => "",
    };
    let query = format!(
        "SELECT fi.id, fi.feed_id, fi.title, fi.link,         fi.published_at_ms,         (es.read_at IS NOT NULL) AS is_read         FROM feed_items fi         LEFT JOIN entry_states es ON es.item_id = fi.id AND es.user_id = ?1         WHERE 1=1 {condition}         ORDER BY fi.id DESC         LIMIT ?2 OFFSET ?3"
    );
    let rows = sqlx::query_as::<_, EntrySummary>(&query)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

async fn mark_entries_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<EntryBatchRequest>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;
    if payload.item_ids.is_empty() {
        return Err(ServerError::new(StatusCode::BAD_REQUEST, "item_ids required"));
    }

    if let Some(pool) = &state.postgres {
        sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at)             SELECT $1, UNNEST($2::BIGINT[]), NOW()             ON CONFLICT (user_id, item_id) DO UPDATE SET read_at = EXCLUDED.read_at",
        )
        .bind(user_id)
        .bind(&payload.item_ids)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(StatusCode::NO_CONTENT);
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    for item_id in payload.item_ids {
        sqlx::query(
            "INSERT INTO entry_states (user_id, item_id, read_at) VALUES (?1, ?2, datetime('now'))             ON CONFLICT(user_id, item_id) DO UPDATE SET read_at = excluded.read_at",
        )
        .bind(user_id)
        .bind(item_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    tx.commit()
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn mark_entries_unread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<EntryBatchRequest>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;
    if payload.item_ids.is_empty() {
        return Err(ServerError::new(StatusCode::BAD_REQUEST, "item_ids required"));
    }

    if let Some(pool) = &state.postgres {
        sqlx::query("DELETE FROM entry_states WHERE user_id = $1 AND item_id = ANY($2)")
            .bind(user_id)
            .bind(&payload.item_ids)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(StatusCode::NO_CONTENT);
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    for item_id in payload.item_ids {
        sqlx::query("DELETE FROM entry_states WHERE user_id = ?1 AND item_id = ?2")
            .bind(user_id)
            .bind(item_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    tx.commit()
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_subscriptions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SubscriptionRow>>, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;

    if let Some(pool) = &state.postgres {
        let rows = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT feed_id FROM subscriptions WHERE user_id = $1 ORDER BY feed_id",
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
    let rows = sqlx::query_as::<_, SubscriptionRow>(
        "SELECT feed_id FROM subscriptions WHERE user_id = ?1 ORDER BY feed_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

async fn create_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SubscriptionRequest>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;
    let feed_id = payload.feed_id.trim();
    if feed_id.is_empty() {
        return Err(ServerError::new(StatusCode::BAD_REQUEST, "feed_id required"));
    }

    if let Some(pool) = &state.postgres {
        sqlx::query(
            "INSERT INTO subscriptions (user_id, feed_id, created_at) VALUES ($1, $2, NOW())",
        )
        .bind(user_id)
        .bind(feed_id)
        .execute(pool)
        .await
        .map_err(|e| map_db_error(e, "subscription create failed"))?;
        return Ok(StatusCode::CREATED);
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    sqlx::query(
        "INSERT INTO subscriptions (user_id, feed_id, created_at) VALUES (?1, ?2, datetime('now'))",
    )
    .bind(user_id)
    .bind(feed_id)
    .execute(pool)
    .await
    .map_err(|e| map_db_error(e, "subscription create failed"))?;
    Ok(StatusCode::CREATED)
}

async fn delete_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<StatusCode, ServerError> {
    let user_id = auth_user_id(&state, &headers).await?;

    if let Some(pool) = &state.postgres {
        sqlx::query("DELETE FROM subscriptions WHERE user_id = $1 AND feed_id = $2")
            .bind(user_id)
            .bind(&feed_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(StatusCode::NO_CONTENT);
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    sqlx::query("DELETE FROM subscriptions WHERE user_id = ?1 AND feed_id = ?2")
        .bind(user_id)
        .bind(&feed_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
