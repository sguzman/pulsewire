mod config;

use std::net::SocketAddr;
use std::path::Path;

use axum::{routing::get, Router};
use config::{validate_schema_name, AppMode, ConfigError, ServerConfig, SqlDialect};
use sqlx::{Pool, Postgres, Sqlite};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

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

    let (sqlite_pool, postgres_pool) = connect_db(&config, Path::new(&config_path)).await?;

    if config.app.mode == AppMode::Dev && config.dev.reset_on_start {
        reset_server_data(&config, &sqlite_pool, &postgres_pool).await?;
    }

    let addr: SocketAddr = format!("{}:{}", config.http.host, config.http.port)
        .parse()
        .map_err(|e| ConfigError::Invalid(format!("invalid http bind: {e}")))?;

    let app = Router::new().route("/health", get(health));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await.map_err(|e| {
        ConfigError::Invalid(format!("http server error: {e}"))
    })?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
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

async fn connect_db(
    config: &ServerConfig,
    config_path: &Path,
) -> Result<(Option<Pool<Sqlite>>, Option<Pool<Postgres>>), ConfigError> {
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
            Ok((Some(pool), None))
        }
        SqlDialect::Postgres => {
            let pg = config
                .postgres
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres section missing".into()))?;
            let schema = validate_schema_name(&pg.schema)?;
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
            Ok((None, Some(pool)))
        }
    }
}

async fn reset_server_data(
    config: &ServerConfig,
    sqlite_pool: &Option<Pool<Sqlite>>,
    postgres_pool: &Option<Pool<Postgres>>,
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
            let pool = sqlite_pool
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
            let pool = postgres_pool
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres pool missing".into()))?;
            let schema = config
                .postgres
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("postgres section missing".into()))?
                .schema
                .as_str();
            let schema = validate_schema_name(schema)?;
            let statements = tables.iter().map(|t| {
                format!(
                    "TRUNCATE TABLE IF EXISTS {}.{} RESTART IDENTITY",
                    quote_ident(&schema),
                    quote_ident(t)
                )
            });
            for stmt in statements {
                sqlx::query(&stmt)
                    .execute(pool)
                    .await
                    .map_err(|e| ConfigError::Invalid(format!("cleanup failed: {e}")))?;
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

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
