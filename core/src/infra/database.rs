//! Database wiring: creates repository implementations per SQL dialect.
use std::sync::Arc;

use crate::domain::model::{AppConfig, SqlDialect};
use crate::infra::{postgres_repo::PostgresRepo, sqlite_repo::SqliteRepo};
use crate::ports::repo::Repo;

pub async fn create_repo(dialect: SqlDialect, cfg: &AppConfig) -> Result<Arc<dyn Repo>, String> {
    match dialect {
        SqlDialect::Sqlite => Ok(Arc::new(SqliteRepo::new(&cfg.sqlite_path).await?)),
        SqlDialect::Postgres => Ok(Arc::new(
            PostgresRepo::new(&cfg.postgres, &cfg.timezone)
                .await
                .map_err(|e| format!("pg repo: {e}"))?,
        )),
    }
}
