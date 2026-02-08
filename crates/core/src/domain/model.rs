//! Domain models: app/feed
//! configuration, HTTP result shapes,
//! and error taxonomy.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono_tz::Tz;
use serde::{
  Deserialize,
  Serialize
};

#[derive(
  Debug, Clone, Serialize, Deserialize,
)]
pub struct DomainConfig {
  pub max_concurrent_requests: usize
}

#[derive(
  Debug, Clone, Serialize, Deserialize,
)]
pub struct CategoryConfig {
  pub name:    String,
  pub domains: Vec<String>
}

#[derive(
  Debug, Clone, Serialize, Deserialize,
)]
pub struct FeedConfig {
  pub id:                String,
  pub url:               String,
  pub domain:            String,
  pub category:          String,
  pub base_poll_seconds: u64,
  pub provenance:        Option<String>,
  pub tags: Option<Vec<String>>,
  pub language:          Option<String>,
  pub content_type:      Option<String>,
  pub cookie_path:       Option<String>,
  pub headers_path:      Option<String>,
  pub headers:
    Option<HashMap<String, String>>
}

#[derive(
  Debug, Clone, Serialize, Deserialize,
)]
pub struct WatchConfig {
  pub id:                    String,
  pub url:                   String,
  pub domain:                String,
  pub category:              String,
  pub base_poll_seconds:     u64,
  pub provenance: Option<String>,
  pub tags: Option<Vec<String>>,
  pub language: Option<String>,
  pub content_type: Option<String>,
  pub cookie_path: Option<String>,
  pub headers_path: Option<String>,
  pub headers:
    Option<HashMap<String, String>>,
  pub check_method: WatchCheckMethod,
  pub fallback_to_get:       bool,
  pub detectors: Vec<WatchDetector>,
  pub fetch_body_on_change:  bool,
  pub max_body_bytes: Option<u64>,
  pub max_items_per_fetch: Option<u64>,
  pub item_selector: Option<String>,
  pub item_identity:
    Option<WatchItemIdentity>,
  pub item_identity_attr:
    Option<String>,
  pub title_selector: Option<String>,
  pub link_selector: Option<String>,
  pub summary_selector: Option<String>,
  pub published_selector:
    Option<String>,
  pub published_format: Option<String>,
  pub include_selectors:
    Option<Vec<String>>,
  pub exclude_selectors:
    Option<Vec<String>>,
  pub normalize_whitespace:  bool,
  pub strip_query_params:    bool,
  pub emit_mode: WatchEmitMode,
  pub emit_title: Option<String>,
  pub min_item_count_change:
    Option<u64>
}

#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Serialize,
  Deserialize,
)]
pub enum WatchCheckMethod {
  Head,
  Get
}

#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Serialize,
  Deserialize,
)]
pub enum WatchDetector {
  Etag,
  LastModified,
  ContentLength,
  ContentHash,
  ElementHash
}

#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Serialize,
  Deserialize,
)]
pub enum WatchItemIdentity {
  Href,
  Text,
  Attr
}

#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Serialize,
  Deserialize,
)]
pub enum WatchEmitMode {
  NewItemsOnly,
  AnyChange,
  Digest
}

#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Serialize,
  Deserialize,
)]
pub enum AppMode {
  Dev,
  Prod
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
  pub immediate_error_statuses:
    Vec<u16>,
  pub jitter_fraction: f64,
  pub global_max_concurrent_requests:
    Option<usize>,
  pub user_agent: String,
  pub log_level: String,
  pub log_file_enabled: bool,
  pub log_file_level: String,
  pub log_file_directory: PathBuf,
  pub log_file_name: String,
  pub log_file_rotation: String,
  pub log_tick_warn_seconds: u64,
  pub log_feed_timing_enabled: bool,
  pub log_feed_timing_domains:
    Vec<String>,
  pub log_feed_timing_warn_ms: u64,
  pub log_feed_timing_log_all: bool,
  pub metrics: MetricsConfig,
  pub mode: AppMode,
  pub timezone: Tz,
  pub domains:
    HashMap<String, DomainConfig>,
  pub state_history_sample_rate: f64
}

#[derive(Debug, Clone)]
pub struct MetricsConfig {
  pub enabled: bool,
  pub bind:    String
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq,
)]
pub enum SqlDialect {
  Sqlite,
  Postgres
}

#[derive(Debug, Clone)]
pub struct PostgresConfig {
  pub user:     String,
  pub password: String,
  pub host:     String,
  pub port:     u16,
  pub database: String,
  pub schema:   String
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq,
)]
pub enum ErrorKind {
  Timeout,
  DnsFailure,
  ConnectionFailure,
  Http4xx(u16),
  Http5xx(u16),
  ParseError,
  Unexpected
}

#[derive(Debug, Clone)]
pub struct HeadResult {
  pub status:             Option<u16>,
  pub etag: Option<String>,
  pub last_modified:      Option<i64>, /* epoch millis */
  pub error: Option<ErrorKind>,
  pub latency_ms:         u64,
  pub set_cookie_headers: Vec<String>
}

#[derive(Debug, Clone)]
pub struct GetResult {
  pub status:             Option<u16>,
  pub body: Option<Vec<u8>>,
  pub etag: Option<String>,
  pub last_modified:      Option<i64>, /* epoch millis */
  pub error: Option<ErrorKind>,
  pub latency_ms:         u64,
  pub set_cookie_headers: Vec<String>
}
