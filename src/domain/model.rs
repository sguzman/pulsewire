use std::{collections::HashMap, path::PathBuf};

use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfig {
    pub max_concurrent_requests: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    pub id: String,
    pub url: String,
    pub domain: String,
    pub base_poll_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppMode {
    Dev,
    Prod,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub db_path: PathBuf,
    pub default_poll_seconds: u64,
    pub max_poll_seconds: u64,
    pub error_backoff_base_seconds: u64,
    pub max_error_backoff_seconds: u64,
    pub jitter_fraction: f64,
    pub global_max_concurrent_requests: Option<usize>,
    pub user_agent: String,
    pub log_level: String,
    pub mode: AppMode,
    pub timezone: Tz,
    pub domains: HashMap<String, DomainConfig>,
    pub feeds: Vec<FeedConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Timeout,
    DnsFailure,
    ConnectionFailure,
    Http4xx,
    Http5xx,
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
