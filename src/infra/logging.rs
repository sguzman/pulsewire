use thiserror::Error;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Error)]
pub enum BootError {
    #[error("fatal: {0}")]
    Fatal(String),
}

pub fn init_logging(level: &str) {
    // Base level from config, still overridable via RUST_LOG.
    let default = format!("{level},feedrv3={level},sqlx=warn,reqwest=warn");
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();
}
