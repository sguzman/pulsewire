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
  let (bootstrap_only, recovery_only) =
    match args.as_slice() {
      | [] => (false, false),
      | [flag]
        if flag
          == "--bootstrap-admin" =>
      {
        (true, false)
      }
      | [flag]
        if flag == "--recover-user" =>
      {
        (false, true)
      }
      | _ => {
        return Err(
          ConfigError::Invalid(
            "usage: pulsewire-server \
             [--bootstrap-admin | \
             --recover-user]"
              .into()
          )
        );
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

  if config.dev.reset_on_start {
    return Err(ConfigError::Invalid(
      "dev.reset_on_start is \
       unsupported: automatic \
       database resets are not safe"
        .into()
    ));
  }

  // Reject missing or invalid operator
  // authorization before opening
  // any database connection or
  // executing schema migrations.
  let recovery_credentials =
    if recovery_only {
      if std::env::var(
        "PULSEWIRE_RECOVERY_CONFIRM"
      )
      .ok()
      .as_deref()
        != Some("RECOVER_USER")
      {
        return Err(ConfigError::Invalid(
        "set PULSEWIRE_RECOVERY_CONFIRM=RECOVER_USER for offline recovery".into()
      ));
      }
      let username = std::env::var("PULSEWIRE_RECOVERY_USERNAME")
      .map_err(|_| ConfigError::Invalid("PULSEWIRE_RECOVERY_USERNAME required".into()))?;
      let password = std::env::var("PULSEWIRE_RECOVERY_PASSWORD")
      .map_err(|_| ConfigError::Invalid("PULSEWIRE_RECOVERY_PASSWORD required".into()))?;
      if username.trim().is_empty()
        || password.len() < 12
      {
        return Err(
          ConfigError::Invalid(
            "recovery requires an \
             existing username and \
             password at least 12 \
             bytes long"
              .into()
          )
        );
      }
      Some((username, password))
    } else {
      None
    };

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

  if let Some((username, password)) =
    recovery_credentials
  {
    db::recover_user_password(
      &state, &username, &password
    )
    .await?;
    tracing::info!(
      "user recovered; all previous \
       sessions revoked; HTTP server \
       not started"
    );
    return Ok(());
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
