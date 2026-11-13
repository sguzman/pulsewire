use std::net::SocketAddr;
use std::path::Path;

use pulsewire_schemas::schema_dir;

use crate::config::{
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
  let args: Vec<String> =
    std::env::args().skip(1).collect();
  let bootstrap_only = match args
    .as_slice()
  {
    | [] => false,
    | [flag]
      if flag
        == "--bootstrap-admin" =>
    {
      true
    }
    | _ => {
      return Err(ConfigError::Invalid(
        "usage: pulsewire-server \
         [--bootstrap-admin]"
          .into()
      ))
    }
  };
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

  if config.dev.reset_on_start {
    return Err(ConfigError::Invalid(
      "dev.reset_on_start is \
       unsupported: automatic \
       database resets are not safe"
        .into()
    ));
  }
  if bootstrap_only {
    let username = std::env::var(
      "PULSEWIRE_BOOTSTRAP_USERNAME"
    )
    .map_err(|_| {
      ConfigError::Invalid(
        "PULSEWIRE_BOOTSTRAP_USERNAME \
         required"
          .into()
      )
    })?;
    let password = std::env::var(
      "PULSEWIRE_BOOTSTRAP_PASSWORD"
    )
    .map_err(|_| {
      ConfigError::Invalid(
        "PULSEWIRE_BOOTSTRAP_PASSWORD \
         required"
          .into()
      )
    })?;
    db::bootstrap_admin(
      &state, &username, &password
    )
    .await?;
    tracing::info!(
      "administrator bootstrapped; \
       server not started"
    );
    return Ok(());
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
