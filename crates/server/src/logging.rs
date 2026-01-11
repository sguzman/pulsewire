use tracing_subscriber::EnvFilter;

use crate::config::{ConfigError, ServerConfig};

pub fn init_tracing(config: &ServerConfig) -> Result<(), ConfigError> {
    let level = config
        .logging
        .level
        .as_deref()
        .unwrap_or("info")
        .trim()
        .to_string();
    let filter = EnvFilter::try_new(level)
        .map_err(|e| ConfigError::Invalid(format!("invalid logging.level: {e}")))?;

    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
