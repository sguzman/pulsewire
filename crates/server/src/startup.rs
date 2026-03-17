use std::net::SocketAddr;
use std::path::Path;

use pulsewire_schemas::schema_dir;

use crate::config::{
  AppMode,
  ConfigError,
  ServerConfig
};
use crate::{
  db,
  handlers,
  logging,
  schema
};

pub async fn run()
-> Result<(), ConfigError> {
  let config_path =
    std::env::var("SERVER_CONFIG_PATH")
      .unwrap_or_else(|_| {
        "crates/server/res/config.toml"
          .to_string()
      });

  unsafe {
    std::env::set_var(
      "SCHEMAS_DIR",
      schema_dir("server")
    );
  }

  let config = ServerConfig::load(
    Path::new(&config_path)
  )
  .await?;

  logging::init_tracing(&config)?;

  if let Some(tz) =
    config.app.timezone.as_deref()
  {
    tracing::info!(
      timezone = tz,
      "server timezone configured"
    );
  }

  tracing::info!(mode = ?config.app.mode, "server mode configured");

  tracing::info!(
    host = %config.http.host,
    port = config.http.port,
    "server http bind"
  );

  tracing::info!(
    "server docs available at /docs \
     and /openapi.json"
  );

  let state = db::connect_db(
    &config,
    Path::new(&config_path)
  )
  .await?;

  schema::apply_server_schema(
    &config,
    &state,
    Path::new(&config_path)
  )
  .await?;

  if config.app.mode == AppMode::Dev
    && config.dev.reset_on_start
  {
    db::reset_server_data(
      &config, &state
    )
    .await?;
  }

  if config.app.mode == AppMode::Dev {
    db::ensure_default_user(
      &config,
      &state,
      &config.seed.username,
      &config.seed.password
    )
    .await?;
  }

  let addr: SocketAddr = format!(
    "{}:{}",
    config.http.host, config.http.port
  )
  .parse()
  .map_err(|e| {
    ConfigError::Invalid(format!(
      "invalid http bind: {e}"
    ))
  })?;

  let app = handlers::router(state);

  let listener =
    tokio::net::TcpListener::bind(addr)
      .await?;

  axum::serve(listener, app)
    .await
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "http server error: {e}"
      ))
    })?;

  Ok(())
}
