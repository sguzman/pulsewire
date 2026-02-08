use serde::Deserialize;

use super::defaults::{
  default_immediate_error_statuses,
  default_log_feed_timing_warn_ms,
  default_log_file_directory,
  default_log_file_enabled,
  default_log_file_level,
  default_log_file_name,
  default_log_file_rotation,
  default_log_tick_warn_seconds,
  default_max_consecutive_errors,
  default_metrics_bind,
  default_metrics_enabled,
  default_pg_database,
  default_pg_host,
  default_pg_password,
  default_pg_port,
  default_pg_schema,
  default_pg_user,
  default_sqlite_path
};

#[derive(Debug, Deserialize)]
pub(crate) struct RawAppFile {
  pub app:           RawApp,
  pub database:      RawDatabase,
  #[serde(default)]
  pub sqlite:        RawSqlite,
  #[serde(default)]
  pub postgres: Option<RawPostgres>,
  pub polling:       RawPolling,
  pub backoff:       RawBackoff,
  pub requests:      RawRequests,
  pub logging:       RawLogging,
  #[serde(default)]
  pub metrics:       Option<RawMetrics>,
  #[serde(default)]
  pub state_history:
    Option<RawStateHistory>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawApp {
  pub mode:     Option<String>,
  pub timezone: Option<String>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawDatabase {
  #[serde(default)]
  pub dialect: Option<String>
}

#[derive(
  Debug, Deserialize, Default,
)]
pub(crate) struct RawSqlite {
  #[serde(
    default = "default_sqlite_path"
  )]
  pub path: String
}

#[derive(
  Debug, Deserialize, Default,
)]
pub(crate) struct RawPostgres {
  #[serde(default = "default_pg_user")]
  pub user:     String,
  #[serde(
    default = "default_pg_password"
  )]
  pub password: String,
  #[serde(default = "default_pg_host")]
  pub host:     String,
  #[serde(default = "default_pg_port")]
  pub port:     u16,
  #[serde(
    default = "default_pg_database"
  )]
  pub db:       String,
  #[serde(
    default = "default_pg_schema"
  )]
  pub schema:   String
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawPolling {
  pub default_seconds: u64,
  pub max_seconds:     u64,
  pub jitter_fraction: f64
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawBackoff {
  pub error_base_seconds:       u64,
  pub max_error_seconds:        u64,
  #[serde(
    default = "default_max_consecutive_errors"
  )]
  pub max_consecutive_errors:   u32,
  #[serde(
    default = "default_immediate_error_statuses"
  )]
  pub immediate_error_statuses:
    Vec<u16>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawRequests {
  pub global_max_concurrent_requests:
    Option<usize>,
  pub user_agent: String
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawLogging {
  pub level: Option<String>,
  #[serde(
    default = "default_log_file_enabled"
  )]
  pub file_enabled:        bool,
  #[serde(
    default = "default_log_file_directory"
  )]
  pub file_directory:      String,
  #[serde(
    default = "default_log_file_name"
  )]
  pub file_name:           String,
  #[serde(
    default = "default_log_file_rotation"
  )]
  pub file_rotation:       String,
  #[serde(
    default = "default_log_file_level"
  )]
  pub file_level:          String,
  #[serde(
    default = "default_log_tick_warn_seconds"
  )]
  pub tick_warn_seconds:   u64,
  #[serde(default)]
  pub feed_timing_enabled: bool,
  #[serde(default)]
  pub feed_timing_domains: Vec<String>,
  #[serde(
    default = "default_log_feed_timing_warn_ms"
  )]
  pub feed_timing_warn_ms: u64,
  #[serde(default)]
  pub feed_timing_log_all: bool
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawMetrics {
  #[serde(
    default = "default_metrics_enabled"
  )]
  pub enabled: bool,
  #[serde(
    default = "default_metrics_bind"
  )]
  pub bind:    String
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawStateHistory {
  pub sample_rate: Option<f64>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawDomainsFile {
  pub domains: Vec<RawDomainEntry>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawDomainEntry {
  pub name:                    String,
  pub max_concurrent_requests: usize
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawFeedsFile {
  pub base_poll_seconds: Option<u64>,
  pub id_prefix:         Option<String>,
  pub category:          Option<String>,
  pub provenance:        Option<String>,
  pub tags: Option<Vec<String>>,
  pub language:          Option<String>,
  pub content_type:      Option<String>,
  #[serde(default)]
  pub feeds:             Vec<RawFeed>,
  pub watch_defaults:
    Option<RawWatchDefaults>,
  #[serde(default)]
  pub watch_profiles:
    Vec<RawWatchProfile>,
  #[serde(default)]
  pub watches:           Vec<RawWatch>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawFeedDefaults {
  pub base_poll_seconds: Option<u64>,
  pub id_prefix:         Option<String>,
  pub category:          Option<String>,
  pub provenance:        Option<String>,
  pub tags: Option<Vec<String>>,
  pub language:          Option<String>,
  pub content_type:      Option<String>,
  pub watch_defaults:
    Option<RawWatchDefaults>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCategoriesFile {
  pub categories: Vec<RawCategoryEntry>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCategoryEntry {
  pub name:    String,
  pub domains: Vec<String>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawFeed {
  pub id:                String,
  pub url:               String,
  pub base_poll_seconds: Option<u64>,
  pub id_prefix:         Option<String>,
  pub category:          Option<String>,
  pub provenance:        Option<String>,
  pub tags: Option<Vec<String>>,
  pub language:          Option<String>,
  pub content_type:      Option<String>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawWatchDefaults {
  pub base_poll_seconds: Option<u64>,
  pub id_prefix: Option<String>,
  pub category: Option<String>,
  pub provenance: Option<String>,
  pub tags: Option<Vec<String>>,
  pub language: Option<String>,
  pub content_type: Option<String>,
  pub check_method: Option<String>,
  pub fallback_to_get: Option<bool>,
  pub detectors: Option<Vec<String>>,
  pub fetch_body_on_change:
    Option<bool>,
  pub max_body_bytes: Option<u64>,
  pub item_selector: Option<String>,
  pub item_identity: Option<String>,
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
  pub normalize_whitespace:
    Option<bool>,
  pub strip_query_params: Option<bool>,
  pub emit_mode: Option<String>,
  pub emit_title: Option<String>,
  pub min_item_count_change:
    Option<u64>
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawWatchProfile {
  pub name:     String,
  #[serde(flatten)]
  pub defaults: RawWatchDefaults
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawWatch {
  pub id:                    String,
  pub url:                   String,
  pub profile: Option<String>,
  pub base_poll_seconds: Option<u64>,
  pub id_prefix: Option<String>,
  pub category: Option<String>,
  pub provenance: Option<String>,
  pub tags: Option<Vec<String>>,
  pub language: Option<String>,
  pub content_type: Option<String>,
  pub check_method: Option<String>,
  pub fallback_to_get: Option<bool>,
  pub detectors: Option<Vec<String>>,
  pub fetch_body_on_change:
    Option<bool>,
  pub max_body_bytes: Option<u64>,
  pub item_selector: Option<String>,
  pub item_identity: Option<String>,
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
  pub normalize_whitespace:
    Option<bool>,
  pub strip_query_params: Option<bool>,
  pub emit_mode: Option<String>,
  pub emit_title: Option<String>,
  pub min_item_count_change:
    Option<u64>
}
