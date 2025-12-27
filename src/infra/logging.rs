//! Logging bootstrap using `tracing` with env-filter override support.
use std::sync::OnceLock;
use thiserror::Error;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, Registry};

use crate::domain::model::AppConfig;

#[derive(Debug, Error)]
pub enum BootError {
    #[error("fatal: {0}")]
    Fatal(String),
}

static LOG_GUARDS: OnceLock<Vec<tracing_appender::non_blocking::WorkerGuard>> = OnceLock::new();

pub fn init_logging(cfg: &AppConfig) {
    // Base level from config, still overridable via RUST_LOG.
    let level = cfg.log_level.trim();
    let default = format!("{level},feedrv3={level},sqlx=warn,reqwest=warn");
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_filter(filter);

    let mut guards = Vec::new();
    let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();
    layers.push(stdout_layer.boxed());

    if cfg.log_file_enabled && cfg.log_file_level != "off" {
        if let Err(e) = std::fs::create_dir_all(&cfg.log_file_directory) {
            eprintln!(
                "failed to create log directory {}: {e}",
                cfg.log_file_directory.display()
            );
        } else {
            let min_level = parse_level(&cfg.log_file_level);
            let rotation = match cfg.log_file_rotation.as_str() {
                "hourly" => Rotation::HOURLY,
                _ => Rotation::HOURLY,
            };
            for level in levels_from(min_level) {
                let prefix = format!("{}-{}", cfg.log_file_name, level.as_str().to_ascii_lowercase());
                let appender = RollingFileAppender::new(
                    rotation.clone(),
                    &cfg.log_file_directory,
                    prefix,
                );
                let (writer, guard) = tracing_appender::non_blocking(appender);
                guards.push(guard);

                let file_layer = tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_target(true)
                    .with_level(true)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_writer(writer)
                    .with_filter(tracing_subscriber::filter::filter_fn(move |meta| {
                        meta.level() == &level
                    }))
                    .boxed();
                layers.push(file_layer);
            }
        }
    }

    let _ = LOG_GUARDS.set(guards);
    tracing_subscriber::registry().with(layers).init();
}

fn parse_level(level: &str) -> Level {
    match level {
        "error" => Level::ERROR,
        "warn" => Level::WARN,
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    }
}

fn levels_from(min_level: Level) -> Vec<Level> {
    let all = [Level::ERROR, Level::WARN, Level::INFO, Level::DEBUG, Level::TRACE];
    let idx = all
        .iter()
        .position(|l| *l == min_level)
        .unwrap_or(2);
    all[..=idx].to_vec()
}
