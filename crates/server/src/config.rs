use std::env;
use std::path::{
  Path,
  PathBuf
};

use serde::Deserialize;
use thiserror::Error;

#[derive(Error)]
pub enum ConfigError {
  #[error("config IO error: {0}")]
  Io(#[from] std::io::Error),
  #[error(
    "config parse error (source \
     omitted to protect secrets)"
  )]
  Parse(#[from] toml::de::Error),
  #[error("config invalid: {0}")]
  Invalid(String)
}

impl std::fmt::Debug for ConfigError {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    write!(f, "{self}")
  }
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
  pub dev:      DevConfig
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

#[derive(Deserialize)]
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

impl std::fmt::Debug
  for PostgresConfig
{
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    f.debug_struct("PostgresConfig")
      .field("host", &self.host)
      .field("port", &self.port)
      .field("database", &self.database)
      .field("user", &self.user)
      .field("password", &"<redacted>")
      .field("ssl_mode", &self.ssl_mode)
      .field("schema", &self.schema)
      .field(
        "fetcher_schema",
        &self.fetcher_schema
      )
      .finish()
  }
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

    let schema_dir =
      env::var("SCHEMAS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
          base_dir.join("schemas")
        });

    let schema_path = schema_dir
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

    let mut config: ServerConfig =
      toml::from_str(&content)?;
    if let Ok(value) = env::var(
      "PULSEWIRE_POSTGRES_HOST"
    ) && !value.trim().is_empty()
      && let Some(pg) =
        config.postgres.as_mut()
    {
      pg.host = value;
    }
    if let Ok(value) = env::var(
      "PULSEWIRE_POSTGRES_PASSWORD"
    ) && !value.trim().is_empty()
      && let Some(pg) =
        config.postgres.as_mut()
    {
      pg.password = value;
    }
    if config.dialect()?
      == SqlDialect::Postgres
      && config
        .postgres
        .as_ref()
        .map(|pg| {
          pg.password.trim().is_empty()
        })
        .unwrap_or(true)
    {
      return Err(
        ConfigError::Invalid(
          "postgres password \
           required: set \
           PULSEWIRE_POSTGRES_PASSWORD \
           or a private config \
           password"
            .into()
        )
      );
    }

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
      .map_err(|_| {
        ConfigError::Invalid(format!(
          "{name}: invalid TOML \
           (source omitted)"
        ))
      })?;

  let json_value =
    serde_json::to_value(toml_value)
      .map_err(|_| {
        ConfigError::Invalid(
          "config conversion failed \
           (values omitted)"
            .into()
        )
      })?;

  // Schema errors may embed rejected
  // values, including credentials.
  if compiled
    .iter_errors(&json_value)
    .next()
    .is_some()
  {
    return Err(ConfigError::Invalid(
      format!(
        "schema validation failed for \
         {name} (values omitted)"
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

#[cfg(test)]
mod secret_debug_tests {
  use super::*;

  #[test]
  fn server_postgres_debug_never_displays_password()
   {
    let config = PostgresConfig {
      host:           "db".into(),
      port:           5432,
      database:       "pulsewire"
        .into(),
      user:           "example".into(),
      password:
        "sensitive-debug-marker".into(),
      ssl_mode:       "disable".into(),
      schema:         "server".into(),
      fetcher_schema: "fetcher".into()
    };
    let printed = format!("{config:?}");
    assert!(
      printed.contains("<redacted>")
    );
    assert!(!printed.contains(
      "sensitive-debug-marker"
    ));
  }
}

#[cfg(test)]
mod validation_redaction_tests {
  use super::*;

  #[test]
  fn toml_and_schema_errors_hide_password_values()
   {
    let secret =
      "pw023-config-secret-sentinel";
    let schema = r#"{"type":"object","properties":{"password":{"type":"integer"}}}"#;
    let invalid = format!(
      "password = \"{secret}\""
    );
    let err = validate_toml(
      schema, &invalid, "server"
    )
    .expect_err(
      "schema validation should \
       reject secret string"
    );
    for message in [
      err.to_string(),
      format!("{err:?}")
    ] {
      assert!(
        message
          .contains("values omitted")
      );
      assert!(
        !message.contains(secret)
      );
    }
    let malformed =
      format!("password = \"{secret}");
    let err = validate_toml(
      schema, &malformed, "server"
    )
    .expect_err("malformed TOML");
    assert!(
      !err.to_string().contains(secret)
    );
    assert!(
      !format!("{err:?}")
        .contains(secret)
    );
    let parse_error = toml::from_str::<
      toml::Value
    >(&malformed)
    .expect_err("malformed TOML");
    let err =
      ConfigError::Parse(parse_error);
    assert!(
      !err.to_string().contains(secret)
    );
    assert!(
      !format!("{err:?}")
        .contains(secret)
    );
  }
}
