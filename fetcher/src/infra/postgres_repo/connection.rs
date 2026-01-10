//! Helpers to create/configure the Postgres pool.
use chrono_tz::Tz;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};

use crate::domain::model::PostgresConfig;

pub async fn create_pool(cfg: &PostgresConfig, timezone: &Tz) -> Result<PgPool, String> {
    let opts = connect_options(cfg, Some(&cfg.database));
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .after_connect(set_time_zone(timezone))
        .connect_with(opts.clone())
        .await;

    match pool {
        Ok(p) => Ok(p),
        Err(_e) => {
            ensure_database_exists(cfg).await?;
            PgPoolOptions::new()
                .max_connections(10)
                .after_connect(set_time_zone(timezone))
                .connect_with(opts)
                .await
                .map_err(|e| format!("postgres connect error after create: {e}"))
        }
    }
}

pub async fn wipe_database(cfg: &PostgresConfig, timezone: &Tz) -> Result<(), String> {
    let pool = create_pool(cfg, timezone).await?;
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(&pool)
        .await
        .map_err(|e| format!("postgres drop schema error: {e}"))?;
    sqlx::query("CREATE SCHEMA public")
        .execute(&pool)
        .await
        .map_err(|e| format!("postgres create schema error: {e}"))?;
    Ok(())
}

fn set_time_zone(
    timezone: &Tz,
) -> impl Fn(
    &mut sqlx::PgConnection,
    sqlx::pool::PoolConnectionMetadata,
)
    -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sqlx::Error>> + Send + '_>> {
    let tz_name = timezone.name().to_string();
    move |conn, _meta| {
        let tz = tz_name.clone();
        Box::pin(async move {
            // Postgres does not accept bind params in SET TIME ZONE; embed the literal safely.
            let stmt = format!("SET TIME ZONE '{}'", tz.replace('\'', "''"));
            sqlx::query(&stmt).execute(conn).await?;
            Ok(())
        })
    }
}

fn connect_options(cfg: &PostgresConfig, database: Option<&str>) -> PgConnectOptions {
    let mut opts = PgConnectOptions::new()
        .host(&cfg.host)
        .port(cfg.port)
        .username(&cfg.user)
        .password(&cfg.password);
    if let Some(db) = database {
        opts = opts.database(db);
    }
    opts
}

async fn ensure_database_exists(cfg: &PostgresConfig) -> Result<(), String> {
    validate_db_name(&cfg.database)?;
    let admin_opts = connect_options(cfg, Some("postgres"));
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(admin_opts)
        .await
        .map_err(|e| format!("postgres connect error (admin db): {e}"))?;

    let create_sql = format!("CREATE DATABASE \"{}\";", &cfg.database);
    let res = sqlx::query(&create_sql).execute(&admin_pool).await;
    match res {
        Ok(_) => Ok(()),
        Err(e) if is_duplicate_db_error(&e) => Ok(()),
        Err(e) => Err(format!("postgres create database error: {e}")),
    }
}

fn validate_db_name(name: &str) -> Result<(), String> {
    if name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        Ok(())
    } else {
        Err(format!(
            "invalid postgres database name '{}': only alphanumeric, '_' and '-' allowed",
            name
        ))
    }
}

fn is_duplicate_db_error(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("42P04"))
}
