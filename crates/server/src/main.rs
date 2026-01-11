mod config;

use std::net::SocketAddr;
use std::path::Path;

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json,
    Router,
};
use config::{validate_schema_name, AppMode, ConfigError, ServerConfig, SqlDialect};
use argon2::{password_hash::{rand_core::OsRng, rand_core::RngCore, SaltString}, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Pool, Postgres, Sqlite};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;
use tokio::fs;


#[derive(Clone)]
struct AppState {
    sqlite: Option<Pool<Sqlite>>,
    postgres: Option<Pool<Postgres>>,
    fetcher_schema: Option<String>,
    token_ttl_seconds: u64,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct FeedSummary {
    id: String,
    url: String,
    domain: String,
    category: String,
    base_poll_seconds: i64,
}


#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: i64,
    username: String,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    token: String,
    token_type: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct SubscriptionRequest {
    feed_id: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SubscriptionRow {
    feed_id: String,
}

#[derive(Debug)]
struct ServerError {
    status: StatusCode,
    message: String,
}

impl ServerError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}
#[tokio::main]
async fn main() -> Result<(), ConfigError> {
    let config_path = std::env::var("SERVER_CONFIG_PATH")
        .unwrap_or_else(|_| "crates/server/res/config.toml".to_string());

    let config = ServerConfig::load(Path::new(&config_path)).await?;
    init_tracing(&config)?;
    if let Some(tz) = config.app.timezone.as_deref() {
        tracing::info!(timezone = tz, "server timezone configured");
    }

    tracing::info!(mode = ?config.app.mode, "server mode configured");
    tracing::info!(host = %config.http.host, port = config.http.port, "server http bind");

    let state = connect_db(&config, Path::new(&config_path)).await?;
    apply_server_schema(&config, &state, Path::new(&config_path)).await?;

    if config.app.mode == AppMode::Dev && config.dev.reset_on_start {
        reset_server_data(&config, &state).await?;
    }

    let addr: SocketAddr = format!("{}:{}", config.http.host, config.http.port)
        .parse()
        .map_err(|e| ConfigError::Invalid(format!("invalid http bind: {e}")))?;

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/feeds", get(list_feeds))
        .route("/v1/users", post(create_user))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/subscriptions", get(list_subscriptions))
        .route("/v1/subscriptions", post(create_subscription))
        .route("/v1/subscriptions/:feed_id", delete(delete_subscription))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await.map_err(|e| {
        ConfigError::Invalid(format!("http server error: {e}"))
    })?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn list_feeds(State(state): State<AppState>) -> Result<Json<Vec<FeedSummary>>, ServerError> {
    if let Some(pool) = &state.postgres {
        let schema = state
            .fetcher_schema
            .as_deref()
            .unwrap_or("fetcher");
        let query = format!(
            "SELECT id, url, domain, category, base_poll_seconds FROM {}.feeds ORDER BY id",
            quote_ident(schema)
        );
        let rows = sqlx::query_as::<_, FeedSummary>(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("feeds query failed: {e}")))?;
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
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("feeds query failed: {e}")))?;
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

    let password_hash = hash_password(password)
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;

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

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ServerError> {
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

async fn auth_user_id(state: &AppState, headers: &HeaderMap) -> Result<i64, ServerError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_token(&token);

    if let Some(pool) = &state.postgres {
        let id = sqlx::query_scalar::<_, i64>(
            "SELECT user_id FROM user_tokens WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid token"))?;
        return Ok(id);
    }

    let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;
    let id = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM user_tokens WHERE token_hash = ?1 AND expires_at > datetime('now')",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| ServerError::new(StatusCode::UNAUTHORIZED, "invalid token"))?;
    Ok(id)
}

fn bearer_token(headers: &HeaderMap) -> Result<String, ServerError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let token = value.strip_prefix("Bearer ").unwrap_or("").trim();
    if token.is_empty() {
        return Err(ServerError::new(StatusCode::UNAUTHORIZED, "missing bearer token"));
    }
    Ok(token.to_string())
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("password hash error: {e}"))?
        .to_string();
    Ok(hash)
}

fn verify_password(hash: &str, password: &str) -> Result<(), String> {
    let parsed = PasswordHash::new(hash).map_err(|e| format!("password hash parse error: {e}"))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|e| format!("password verify error: {e}"))
}

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn map_db_error(err: sqlx::Error, message: &str) -> ServerError {
    if is_unique_violation(&err) {
        return ServerError::new(StatusCode::CONFLICT, message);
    }
    ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db_err)
            if db_err.code().as_deref() == Some("23505")
                || db_err.code().as_deref() == Some("2067")
    )
}

fn init_tracing(config: &ServerConfig) -> Result<(), ConfigError> {
    let level = config
        .logging
        .level
        .as_deref()
        .unwrap_or("info")
        .trim()
        .to_string();
    let filter = EnvFilter::try_new(level)
        .map_err(|e| ConfigError::Invalid(format!("invalid logging.level: {e}")))?;

    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}


async fn apply_server_schema(
    config: &ServerConfig,
    state: &AppState,
    config_path: &Path,
) -> Result<(), ConfigError> {
    let base_dir = config_path
        .parent()
        .ok_or_else(|| ConfigError::Invalid("config path has no parent".into()))?;
    match config.dialect()? {
        SqlDialect::Sqlite => {
            let pool = state
                .sqlite
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("sqlite pool missing".into()))?;
            let schema_path = base_dir.join("sql").join("sqlite").join("schema.sql");
            let content = fs::read_to_string(&schema_path).await.map_err(|_| {
                ConfigError::Invalid(format!("schema not found at {}", schema_path.display()))
            })?;
            execute_schema_sqlite(pool, &content).await?;
        }
        SqlDialect::Postgres => {
            let pool = state
                .postgres
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres pool missing".into()))?;
            let pg = config
                .postgres
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres section missing".into()))?;
            let schema = validate_schema_name(&pg.schema)?;
            let fetcher_schema = validate_schema_name(&pg.fetcher_schema)?;
            let schema_path = base_dir.join("sql").join("postgres").join("schema.sql");
            let content = fs::read_to_string(&schema_path).await.map_err(|_| {
                ConfigError::Invalid(format!("schema not found at {}", schema_path.display()))
            })?;
            execute_schema_postgres(pool, &content, &schema, &fetcher_schema).await?;
        }
    }
    Ok(())
}


async fn execute_schema_sqlite(
    pool: &sqlx::Pool<Sqlite>,
    content: &str,
) -> Result<(), ConfigError> {
    for stmt in content.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query(trimmed)
            .execute(pool)
            .await
            .map_err(|e| ConfigError::Invalid(format!("schema apply error: {e}")))?;
    }
    Ok(())
}

async fn execute_schema_postgres(
    pool: &sqlx::Pool<Postgres>,
    content: &str,
    schema: &str,
    fetcher_schema: &str,
) -> Result<(), ConfigError> {
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| ConfigError::Invalid(format!("schema apply error: {e}")))?;
    let search_stmt = format!(
        "SET search_path TO {}, {}",
        quote_ident(schema),
        quote_ident(fetcher_schema)
    );
    sqlx::query(&search_stmt)
        .execute(&mut *conn)
        .await
        .map_err(|e| ConfigError::Invalid(format!("schema apply error: {e}")))?;

    for stmt in content.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query(trimmed)
            .execute(&mut *conn)
            .await
            .map_err(|e| ConfigError::Invalid(format!("schema apply error: {e}")))?;
    }
    Ok(())
}

async fn connect_db(
    config: &ServerConfig,
    config_path: &Path,
) -> Result<AppState, ConfigError> {
    match config.dialect()? {
        SqlDialect::Sqlite => {
            let base_dir = config_path
                .parent()
                .ok_or_else(|| ConfigError::Invalid("config path has no parent".into()))?;
            let path = config.sqlite_path(base_dir);
            let url = format!("sqlite://{}", path.display());
            let pool = sqlx::SqlitePool::connect(&url)
                .await
                .map_err(|e| ConfigError::Invalid(format!("sqlite connect failed: {e}")))?;
            Ok(AppState {
                sqlite: Some(pool),
                postgres: None,
                fetcher_schema: None,
                token_ttl_seconds: config.auth.token_ttl_seconds,
            })
        }
        SqlDialect::Postgres => {
            let pg = config
                .postgres
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres section missing".into()))?;
            let schema = validate_schema_name(&pg.schema)?;
            let fetcher_schema = validate_schema_name(&pg.fetcher_schema)?;
            let url = format!(
                "postgres://{}:{}@{}:{}/{}?sslmode={}",
                pg.user, pg.password, pg.host, pg.port, pg.database, pg.ssl_mode
            );
            let pool = PgPoolOptions::new()
                .max_connections(10)
                .after_connect(set_search_path(&schema))
                .connect(&url)
                .await
                .map_err(|e| ConfigError::Invalid(format!("postgres connect failed: {e}")))?;
            Ok(AppState {
                sqlite: None,
                postgres: Some(pool),
                fetcher_schema: Some(fetcher_schema),
                token_ttl_seconds: config.auth.token_ttl_seconds,
            })
        }
    }
}

async fn reset_server_data(
    config: &ServerConfig,
    state: &AppState,
) -> Result<(), ConfigError> {
    let tables = [
        "user_tokens",
        "favorites",
        "entry_states",
        "folder_feeds",
        "folders",
        "subscriptions",
        "users",
    ];

    match config.dialect()? {
        SqlDialect::Sqlite => {
            let pool = state.sqlite
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("sqlite pool missing".into()))?;
            for table in tables {
                let query = format!("DELETE FROM {table}");
                if let Err(e) = sqlx::query(&query).execute(pool).await {
                    if !is_missing_table_error(&e) {
                        return Err(ConfigError::Invalid(format!("cleanup {table} failed: {e}")));
                    }
                }
            }
        }
        SqlDialect::Postgres => {
            let pool = state.postgres
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres pool missing".into()))?;
            let schema = config
                .postgres
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres section missing".into()))?
                .schema
                .as_str();
            let schema = validate_schema_name(schema)?;
            for table in tables {
                let stmt = format!(
                    "TRUNCATE TABLE {}.{} RESTART IDENTITY",
                    quote_ident(&schema),
                    quote_ident(table)
                );
                if let Err(e) = sqlx::query(&stmt).execute(pool).await {
                    if !is_missing_table_error(&e) {
                        return Err(ConfigError::Invalid(format!("cleanup failed: {e}")));
                    }
                }
            }
        }
    }

    Ok(())
}

fn set_search_path(
    schema: &str,
) -> impl Fn(
    &mut sqlx::PgConnection,
    sqlx::pool::PoolConnectionMetadata,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sqlx::Error>> + Send + '_>> {
    let schema_name = schema.to_string();
    move |conn, _meta| {
        let schema_copy = schema_name.clone();
        Box::pin(async move {
            let schema_ident = quote_ident(&schema_copy);
            let create_stmt = format!("CREATE SCHEMA IF NOT EXISTS {schema_ident}");
            sqlx::query(&create_stmt).execute(&mut *conn).await?;
            let search_stmt = format!("SET search_path TO {schema_ident}");
            sqlx::query(&search_stmt).execute(&mut *conn).await?;
            Ok(())
        })
    }
}


fn is_missing_table_error(e: &sqlx::Error) -> bool {
    matches!(
        e,
        sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("42P01")
    )
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
