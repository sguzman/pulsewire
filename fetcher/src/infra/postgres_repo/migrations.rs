//! Database migrations for Postgres: create tables/indexes.
use sqlx::PgPool;
use tracing::info;

use super::util::chunk_statements;

const POSTGRES_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/res/sql/postgres/schema.sql"
));

pub async fn migrate(pool: &PgPool, default_poll_seconds: u64) -> Result<(), String> {
    info!("DB migrate start (postgres)");

    for ddl in chunk_statements(POSTGRES_SCHEMA) {
        let stmt = ddl.replace("{default_poll_seconds}", &default_poll_seconds.to_string());
        sqlx::query(&stmt)
            .execute(pool)
            .await
            .map_err(|e| format!("migrate error (ddl): {e}"))?;
    }

    info!("DB migrate done");
    Ok(())
}
