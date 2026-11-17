use super::ConfigError;
use super::raw::RawPostgres;
use crate::domain::model::{
  AppMode,
  PostgresConfig,
  SqlDialect
};

pub(crate) fn parse_dialect(
  s: Option<&str>
) -> Result<SqlDialect, ConfigError> {
  match s
    .map(|x| x.to_ascii_lowercase())
  {
    | None => Ok(SqlDialect::Sqlite),
    | Some(d) if d == "sqlite" => {
      Ok(SqlDialect::Sqlite)
    }
    | Some(d) if d == "postgres" => {
      Ok(SqlDialect::Postgres)
    }
    | Some(other) => {
      Err(ConfigError::Invalid(
        format!(
          "invalid database.dialect \
           '{other}', expected \
           'sqlite' or 'postgres'"
        )
      ))
    }
  }
}

pub(crate) fn parse_postgres(
  raw: Option<RawPostgres>
) -> Result<PostgresConfig, ConfigError>
{
  let pg = raw.unwrap_or_default();

  let schema =
    validate_schema_name(&pg.schema)?;

  Ok(PostgresConfig {
    user: pg.user,
    password: pg.password,
    host: pg.host,
    port: pg.port,
    database: pg.db,
    schema
  })
}

/// Only validate PostgreSQL-specific settings when PostgreSQL is selected.
/// SQLite can omit [postgres], including its schema and credentials.
pub(crate) fn parse_postgres_for_dialect(
  dialect: SqlDialect,
  raw: Option<RawPostgres>
) -> Result<PostgresConfig, ConfigError> {
  if matches!(dialect, SqlDialect::Sqlite) {
    return parse_postgres(None);
  }

  parse_postgres(raw)
}

fn validate_schema_name(
  raw: &str
) -> Result<String, ConfigError> {
  let trimmed = raw.trim();

  if trimmed.is_empty() {
    return Err(ConfigError::Invalid(
      "postgres schema cannot be empty"
        .into()
    ));
  }

  if !trimmed.chars().all(|c| {
    c.is_ascii_alphanumeric()
      || c == '_'
  }) {
    return Err(ConfigError::Invalid(
      format!(
        "invalid postgres schema \
         '{trimmed}': only \
         alphanumeric and '_' allowed"
      )
    ));
  }

  Ok(trimmed.to_string())
}

pub(crate) fn parse_mode(
  s: Option<&str>
) -> Result<AppMode, ConfigError> {
  match s
    .map(|x| x.to_ascii_lowercase())
  {
    | None => Ok(AppMode::Prod),
    | Some(m) if m == "prod" => {
      Ok(AppMode::Prod)
    }
    | Some(m) if m == "dev" => {
      Ok(AppMode::Dev)
    }
    | Some(other) => {
      Err(ConfigError::Invalid(
        format!(
          "invalid app.mode \
           '{other}', expected 'dev' \
           or 'prod'"
        )
      ))
    }
  }
}

pub(crate) fn url_host(
  url: &str
) -> Option<String> {
  crate::infra::http_policy::parse_source_url(
    url,
    crate::infra::http_policy::allow_private_feed_networks()
  )
  .ok()?
  .host_str()
  .map(str::to_string)
}

#[cfg(test)]
mod postgres_dialect_tests {
  use super::*;

  #[test]
  fn sqlite_works_without_postgres_section() {
    let postgres =
      parse_postgres_for_dialect(SqlDialect::Sqlite, None)
        .expect("SQLite does not require [postgres]");
    assert_eq!(postgres.schema, "fetcher");
    assert_eq!(postgres.port, 5432);
  }

  #[test]
  fn sqlite_ignores_unused_postgres_schema() {
    let raw = RawPostgres {
      schema: "not-valid!".into(),
      ..RawPostgres::default()
    };
    assert!(parse_postgres_for_dialect(
      SqlDialect::Sqlite,
      Some(raw)
    ).is_ok());
  }

  #[test]
  fn postgres_still_rejects_invalid_schema() {
    let raw = RawPostgres {
      schema: "not-valid!".into(),
      ..RawPostgres::default()
    };
    assert!(parse_postgres_for_dialect(
      SqlDialect::Postgres,
      Some(raw)
    ).is_err());
  }
}
