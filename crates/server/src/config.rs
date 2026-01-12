use std::path::{
  Path,
  PathBuf
};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]

pub enum ConfigError {
  #[error("config IO error: {0}")]
  Io(#[from] std::io::Error),
  #[error("config parse error: {0}")]
  Parse(#[from] toml::de::Error),
  #[error("config invalid: {0}")]
  Invalid(String)
}

#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Deserialize,
)]
#[serde(rename_all = "lowercase")]

pub enum AppMode {
  Dev,
  Prod
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq,
)]

pub enum SqlDialect {
  Sqlite,
  Postgres
}

#[derive(Debug, Deserialize)]

pub struct ServerConfig {
  pub app:      AppConfig,
  pub http:     HttpConfig,
  pub database: DatabaseConfig,
  pub sqlite:   SqliteConfig,
  pub postgres: Option<PostgresConfig>,
  pub logging:  LoggingConfig,
  pub auth:     AuthConfig,
  pub dev:      DevConfig,
  pub seed:     SeedConfig
}

#[derive(Debug, Deserialize)]

pub struct AppConfig {
  pub mode:     AppMode,
  pub timezone: Option<String>
}

#[derive(Debug, Deserialize)]

pub struct HttpConfig {
  pub host: String,
  pub port: u16
}

#[derive(Debug, Deserialize)]

pub struct DatabaseConfig {
  pub dialect: String
}

#[derive(Debug, Deserialize)]

pub struct SqliteConfig {
  pub path: String
}

#[derive(Debug, Deserialize)]

pub struct PostgresConfig {
  pub host:           String,
  pub port:           u16,
  pub database:       String,
  pub user:           String,
  pub password:       String,
  pub ssl_mode:       String,
  pub schema:         String,
  pub fetcher_schema: String
}

#[derive(Debug, Deserialize)]

pub struct LoggingConfig {
  pub level: Option<String>
}

#[derive(Debug, Deserialize)]

pub struct AuthConfig {
  pub token_ttl_seconds: u64
}

#[derive(Debug, Deserialize)]

pub struct DevConfig {
  pub reset_on_start: bool
}

#[derive(Debug, Deserialize)]

pub struct SeedConfig {
  pub username: String,
  pub password: String
}

impl ServerConfig {
  pub async fn load(
    path: &Path
  ) -> Result<Self, ConfigError> {
    let base_dir = path
      .parent()
      .ok_or_else(|| {
        ConfigError::Invalid(
          "config path has no parent"
            .into()
        )
      })?;

    let schema_path = base_dir
      .join("schemas")
      .join("server.schema.json");

    let schema =
      load_schema(&schema_path).await?;

    let content =
      tokio::fs::read_to_string(path)
        .await?;

    validate_toml(
      &schema,
      &content,
      &path.display().to_string()
    )?;

    let config: ServerConfig =
      toml::from_str(&content)?;

    Ok(config)
  }

  pub fn dialect(
    &self
  ) -> Result<SqlDialect, ConfigError>
  {
    match self
      .database
      .dialect
      .trim()
      .to_lowercase()
      .as_str()
    {
      | "sqlite" => {
        Ok(SqlDialect::Sqlite)
      }
      | "postgres" => {
        Ok(SqlDialect::Postgres)
      }
      | other => {
        Err(ConfigError::Invalid(
          format!(
            "invalid database.dialect \
             '{other}'"
          )
        ))
      }
    }
  }

  pub fn sqlite_path(
    &self,
    base_dir: &Path
  ) -> PathBuf {
    let raw = self.sqlite.path.trim();

    if raw.is_empty() {
      return base_dir
        .join("server.sqlite");
    }

    base_dir.join(raw)
  }
}

async fn load_schema(
  path: &Path
) -> Result<String, ConfigError> {
  let content =
    tokio::fs::read_to_string(path)
      .await
      .map_err(|_| {
        ConfigError::Invalid(format!(
          "schema not found at {}",
          path.display()
        ))
      })?;

  Ok(content)
}

fn validate_toml(
  schema: &str,
  toml_input: &str,
  name: &str
) -> Result<(), ConfigError> {
  let schema_json: serde_json::Value =
    serde_json::from_str(schema)
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "schema parse error: {e}"
        ))
      })?;

  let compiled =
    jsonschema::validator_for(
      &schema_json
    )
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "schema compile error: {e}"
      ))
    })?;

  let toml_value: toml::Value =
    toml::from_str(toml_input)
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "{name}: {e}"
        ))
      })?;

  let json_value =
    serde_json::to_value(toml_value)
      .map_err(|e| {
        ConfigError::Invalid(
          e.to_string()
        )
      })?;

  let mut errors =
    compiled.iter_errors(&json_value);

  if let Some(err) = errors.next() {
    let mut messages =
      vec![err.to_string()];

    for e in errors.take(4) {
      messages.push(e.to_string());
    }

    return Err(ConfigError::Invalid(
      format!(
        "schema validation failed for \
         {name}: {}",
        messages.join("; ")
      )
    ));
  }

  Ok(())
}

pub(crate) fn validate_schema_name(
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
