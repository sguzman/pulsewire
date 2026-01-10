use crate::domain::model::{AppMode, PostgresConfig, SqlDialect};

use super::raw::RawPostgres;
use super::ConfigError;

pub(crate) fn parse_dialect(s: Option<&str>) -> Result<SqlDialect, ConfigError> {
    match s.map(|x| x.to_ascii_lowercase()) {
        None => Ok(SqlDialect::Sqlite),
        Some(d) if d == "sqlite" => Ok(SqlDialect::Sqlite),
        Some(d) if d == "postgres" => Ok(SqlDialect::Postgres),
        Some(other) => Err(ConfigError::Invalid(format!(
            "invalid database.dialect '{other}', expected 'sqlite' or 'postgres'"
        ))),
    }
}

pub(crate) fn parse_postgres(raw: Option<RawPostgres>) -> Result<PostgresConfig, ConfigError> {
    let pg = raw.unwrap_or_default();
    Ok(PostgresConfig {
        user: pg.user,
        password: pg.password,
        host: pg.host,
        port: pg.port,
        database: pg.db,
    })
}

pub(crate) fn parse_mode(s: Option<&str>) -> Result<AppMode, ConfigError> {
    match s.map(|x| x.to_ascii_lowercase()) {
        None => Ok(AppMode::Prod),
        Some(m) if m == "prod" => Ok(AppMode::Prod),
        Some(m) if m == "dev" => Ok(AppMode::Dev),
        Some(other) => Err(ConfigError::Invalid(format!(
            "invalid app.mode '{other}', expected 'dev' or 'prod'"
        ))),
    }
}

pub(crate) fn url_host(url: &str) -> Option<String> {
    // Minimal, dependency-free host extraction.
    // If you prefer stricter parsing, add `url = "2"` and use `Url::parse`.
    let u = url.trim();
    let after_scheme = u.split("://").nth(1)?;
    let host_port = after_scheme.split('/').next()?;
    let host = host_port.split('@').last().unwrap_or(host_port);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}
