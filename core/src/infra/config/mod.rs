//! Loads the TOML configuration bundle (app/domains/feeds) and normalizes it into `AppConfig` + feed list.
mod defaults;
mod feeds;
mod parse;
mod paths;
mod raw;
mod schema;

use std::{collections::HashMap, path::Path};

use chrono_tz::Tz;
use tokio::fs;

use crate::domain::model::{
    AppConfig, CategoryConfig, DomainConfig, FeedConfig, MetricsConfig, PostgresConfig,
};

use defaults::{
    normalize_domains, normalize_log_level, normalize_log_rotation, normalize_status_codes,
};
use feeds::load_all_feeds;
use parse::{parse_dialect, parse_mode, parse_postgres, url_host};
use paths::{resolve_db_base_dir, resolve_log_dir};
use raw::{RawAppFile, RawCategoriesFile, RawDomainsFile};
use schema::{load_schema, validate_toml};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid config: {0}")]
    Invalid(String),
}

pub struct ConfigLoader;

pub struct LoadedConfig {
    pub app: AppConfig,
    pub feeds: Vec<FeedConfig>,
    pub categories: Vec<CategoryConfig>,
}

pub fn validate_semantic(
    app: &AppConfig,
    categories: &[CategoryConfig],
) -> Result<(), ConfigError> {
    let mut category_domains = std::collections::HashSet::new();
    for c in categories {
        for d in &c.domains {
            category_domains.insert(d.as_str());
        }
    }
    for domain in app.domains.keys() {
        if !category_domains.contains(domain.as_str()) {
            return Err(ConfigError::Invalid(format!(
                "domain '{domain}' missing from categories.toml"
            )));
        }
    }
    Ok(())
}

impl ConfigLoader {
    pub async fn load(config_path: &Path) -> Result<LoadedConfig, ConfigError> {
        let default_timezone = "America/Mexico_City";

        let base_dir = config_path
            .parent()
            .ok_or_else(|| ConfigError::Invalid("config path has no parent".into()))?;
        let domains_path = base_dir.join("domains.toml");
        let categories_path = base_dir.join("categories.toml");
        let feeds_dir = match std::env::var("FEEDS_DIR") {
            Ok(p) if !p.trim().is_empty() => Path::new(p.trim()).to_path_buf(),
            _ => base_dir.join("feeds"),
        };
        let schema_dir = base_dir.join("schemas");

        let config_schema = load_schema(&schema_dir, "config.schema.json").await?;
        let domains_schema = load_schema(&schema_dir, "domains.schema.json").await?;
        let categories_schema = load_schema(&schema_dir, "categories.schema.json").await?;
        let feeds_schema = load_schema(&schema_dir, "feeds.schema.json").await?;

        let app_content = fs::read_to_string(config_path).await?;
        validate_toml(
            &config_schema,
            &app_content,
            &config_path.display().to_string(),
        )?;
        let raw_cfg: RawAppFile = toml::from_str(&app_content)?;

        let domains_content = fs::read_to_string(&domains_path).await?;
        validate_toml(
            &domains_schema,
            &domains_content,
            &domains_path.display().to_string(),
        )?;
        let raw_domains: RawDomainsFile = toml::from_str(&domains_content)?;

        let categories_content = fs::read_to_string(&categories_path).await?;
        validate_toml(
            &categories_schema,
            &categories_content,
            &categories_path.display().to_string(),
        )?;
        let raw_categories: RawCategoriesFile = toml::from_str(&categories_content)?;

        let raw_feeds = load_all_feeds(&feeds_dir, &feeds_schema).await?;

        let mode = parse_mode(raw_cfg.app.mode.as_deref())?;
        let tz_str = raw_cfg
            .app
            .timezone
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(default_timezone);
        let timezone: Tz = tz_str
            .parse()
            .map_err(|_| ConfigError::Invalid(format!("invalid timezone '{tz_str}'")))?;
        let log_level = raw_cfg
            .logging
            .level
            .clone()
            .unwrap_or_else(|| "info".to_string());
        let log_file_level = normalize_log_level(&raw_cfg.logging.file_level)?;
        let log_file_rotation = normalize_log_rotation(&raw_cfg.logging.file_rotation)?;
        let log_dir = resolve_log_dir(config_path, &raw_cfg.logging.file_directory);
        let feed_timing_domains = normalize_domains(&raw_cfg.logging.feed_timing_domains)?;
        let immediate_error_statuses =
            normalize_status_codes(&raw_cfg.backoff.immediate_error_statuses)?;

        let db_base = resolve_db_base_dir(config_path);
        let db_path = db_base.join(raw_cfg.sqlite.path);
        let db_dialect = parse_dialect(raw_cfg.database.dialect.as_deref())?;
        let postgres: PostgresConfig = parse_postgres(raw_cfg.postgres)?;

        let mut domains = HashMap::new();
        for d in raw_domains.domains {
            domains.insert(
                d.name,
                DomainConfig {
                    max_concurrent_requests: d.max_concurrent_requests,
                },
            );
        }

        let mut category_names = std::collections::HashSet::new();
        let mut domain_to_category = HashMap::new();
        let mut categories = Vec::new();
        for c in raw_categories.categories {
            let name = c.name.trim().to_string();
            if name.is_empty() {
                return Err(ConfigError::Invalid("category name cannot be empty".into()));
            }
            if !category_names.insert(name.clone()) {
                return Err(ConfigError::Invalid(format!(
                    "duplicate category name '{name}'"
                )));
            }
            let mut domains_vec = Vec::new();
            for d in c.domains {
                let domain = d.trim().to_ascii_lowercase();
                if domain.is_empty() {
                    return Err(ConfigError::Invalid(format!(
                        "category '{name}' has empty domain"
                    )));
                }
                if domain_to_category.insert(domain.clone(), name.clone()).is_some() {
                    return Err(ConfigError::Invalid(format!(
                        "domain '{domain}' appears in multiple categories"
                    )));
                }
                domains_vec.push(domain);
            }
            categories.push(CategoryConfig {
                name,
                domains: domains_vec,
            });
        }
        if categories.is_empty() {
            return Err(ConfigError::Invalid(
                "categories.toml must define at least one category".into(),
            ));
        }

        let history_sample_rate = raw_cfg
            .state_history
            .as_ref()
            .and_then(|s| s.sample_rate)
            .unwrap_or(1.0);
        if !(0.0..=1.0).contains(&history_sample_rate) {
            return Err(ConfigError::Invalid(format!(
                "state_history.sample_rate must be between 0 and 1, got {history_sample_rate}"
            )));
        }

        let mut feeds = Vec::new();
        for f in raw_feeds.feeds {
            let domain = url_host(&f.url)
                .ok_or_else(|| ConfigError::Invalid(format!("feed '{}' missing host", f.id)))?;
            let domain = domain.to_ascii_lowercase();
            let category = domain_to_category.get(&domain).cloned().ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "feed '{}' domain '{domain}' missing from categories",
                    f.id
                ))
            })?;
            feeds.push(FeedConfig {
                id: f.id,
                url: f.url,
                domain,
                category,
                base_poll_seconds: f
                    .base_poll_seconds
                    .unwrap_or(raw_cfg.polling.default_seconds),
            });
        }

        let metrics_cfg = raw_cfg.metrics.unwrap_or(raw::RawMetrics {
            enabled: defaults::default_metrics_enabled(),
            bind: defaults::default_metrics_bind(),
        });

        Ok(LoadedConfig {
            app: AppConfig {
                db_dialect,
                sqlite_path: db_path,
                postgres,
                default_poll_seconds: raw_cfg.polling.default_seconds,
                max_poll_seconds: raw_cfg.polling.max_seconds,
                error_backoff_base_seconds: raw_cfg.backoff.error_base_seconds,
                max_error_backoff_seconds: raw_cfg.backoff.max_error_seconds,
                max_consecutive_errors: raw_cfg.backoff.max_consecutive_errors,
                immediate_error_statuses,
                jitter_fraction: raw_cfg.polling.jitter_fraction,
                global_max_concurrent_requests: raw_cfg.requests.global_max_concurrent_requests,
                user_agent: raw_cfg.requests.user_agent,
                log_level,
                log_file_enabled: raw_cfg.logging.file_enabled,
                log_file_level,
                log_file_directory: log_dir,
                log_file_name: raw_cfg.logging.file_name,
                log_file_rotation,
                log_tick_warn_seconds: raw_cfg.logging.tick_warn_seconds,
                log_feed_timing_enabled: raw_cfg.logging.feed_timing_enabled,
                log_feed_timing_domains: feed_timing_domains,
                log_feed_timing_warn_ms: raw_cfg.logging.feed_timing_warn_ms,
                log_feed_timing_log_all: raw_cfg.logging.feed_timing_log_all,
                metrics: MetricsConfig {
                    enabled: metrics_cfg.enabled,
                    bind: metrics_cfg.bind,
                },
                mode,
                timezone,
                domains,
                state_history_sample_rate: history_sample_rate,
            },
            feeds,
            categories,
        })
    }
}
