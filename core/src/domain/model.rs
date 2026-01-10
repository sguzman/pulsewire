//! Domain models: app/feed configuration, HTTP result shapes, and error taxonomy.
use std::{collections::HashMap, path::PathBuf};

use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfig {
    pub max_concurrent_requests: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryConfig {
    pub name: String,
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    pub id: String,
    pub url: String,
    pub domain: String,
    pub category: String,
    pub base_poll_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppMode {
    Dev,
    Prod,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub db_dialect: SqlDialect,
    pub sqlite_path: PathBuf,
    pub postgres: PostgresConfig,
    pub default_poll_seconds: u64,
    pub max_poll_seconds: u64,
    pub error_backoff_base_seconds: u64,
    pub max_error_backoff_seconds: u64,
    pub max_consecutive_errors: u32,
    pub immediate_error_statuses: Vec<u16>,
    pub jitter_fraction: f64,
    pub global_max_concurrent_requests: Option<usize>,
    pub user_agent: String,
    pub log_level: String,
    pub log_file_enabled: bool,
    pub log_file_level: String,
    pub log_file_directory: PathBuf,
    pub log_file_name: String,
    pub log_file_rotation: String,
    pub log_tick_warn_seconds: u64,
    pub log_feed_timing_enabled: bool,
    pub log_feed_timing_domains: Vec<String>,
    pub log_feed_timing_warn_ms: u64,
    pub log_feed_timing_log_all: bool,
    pub metrics: MetricsConfig,
    pub mode: AppMode,
    pub timezone: Tz,
    pub domains: HashMap<String, DomainConfig>,
    pub state_history_sample_rate: f64,
}

#[derive(Debug, Clone)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub bind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Timeout,
    DnsFailure,
    ConnectionFailure,
    Http4xx(u16),
    Http5xx(u16),
    ParseError,
    Unexpected,
}

#[derive(Debug, Clone)]
pub struct HeadResult {
    pub status: Option<u16>,
    pub etag: Option<String>,
    pub last_modified: Option<i64>, // epoch millis
    pub error: Option<ErrorKind>,
    pub latency_ms: u64,
}

#[derive(Debug, Clone)]
pub struct GetResult {
    pub status: Option<u16>,
    pub body: Option<Vec<u8>>,
    pub etag: Option<String>,
    pub last_modified: Option<i64>, // epoch millis
    pub error: Option<ErrorKind>,
    pub latency_ms: u64,
}
