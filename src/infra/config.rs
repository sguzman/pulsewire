//! Loads the TOML configuration bundle (app/domains/feeds) and normalizes it into `AppConfig` + feed list.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use chrono_tz::Tz;
use serde::Deserialize;
use tokio::fs;

use crate::domain::model::{
    AppConfig, AppMode, CategoryConfig, DomainConfig, FeedConfig, PostgresConfig, SqlDialect,
};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid config: {0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize)]
struct RawAppFile {
    app: RawApp,
    database: RawDatabase,
    #[serde(default)]
    sqlite: RawSqlite,
    #[serde(default)]
    postgres: Option<RawPostgres>,
    polling: RawPolling,
    backoff: RawBackoff,
    requests: RawRequests,
    logging: RawLogging,
    #[serde(default)]
    state_history: Option<RawStateHistory>,
}

#[derive(Debug, Deserialize)]
struct RawApp {
    mode: Option<String>,
    timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawDatabase {
    #[serde(default)]
    dialect: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawSqlite {
    #[serde(default = "default_sqlite_path")]
    path: String,
}

#[derive(Debug, Deserialize, Default)]
struct RawPostgres {
    #[serde(default = "default_pg_user")]
    user: String,
    #[serde(default = "default_pg_password")]
    password: String,
    #[serde(default = "default_pg_host")]
    host: String,
    #[serde(default = "default_pg_port")]
    port: u16,
    #[serde(default = "default_pg_database")]
    db: String,
}

#[derive(Debug, Deserialize)]
struct RawPolling {
    default_seconds: u64,
    max_seconds: u64,
    jitter_fraction: f64,
}

#[derive(Debug, Deserialize)]
struct RawBackoff {
    error_base_seconds: u64,
    max_error_seconds: u64,
    #[serde(default = "default_max_consecutive_errors")]
    max_consecutive_errors: u32,
}

#[derive(Debug, Deserialize)]
struct RawRequests {
    global_max_concurrent_requests: Option<usize>,
    user_agent: String,
}

#[derive(Debug, Deserialize)]
struct RawLogging {
    level: Option<String>,
    #[serde(default = "default_log_file_directory")]
    file_directory: String,
    #[serde(default = "default_log_file_name")]
    file_name: String,
    #[serde(default = "default_log_file_rotation")]
    file_rotation: String,
    #[serde(default = "default_log_file_level")]
    file_level: String,
}

#[derive(Debug, Deserialize)]
struct RawStateHistory {
    sample_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawDomainsFile {
    domains: Vec<RawDomainEntry>,
}

#[derive(Debug, Deserialize)]
struct RawDomainEntry {
    name: String,
    max_concurrent_requests: usize,
}

#[derive(Debug, Deserialize)]
struct RawFeedsFile {
    feeds: Vec<RawFeed>,
}

#[derive(Debug, Deserialize)]
struct RawCategoriesFile {
    categories: Vec<RawCategoryEntry>,
}

#[derive(Debug, Deserialize)]
struct RawCategoryEntry {
    name: String,
    domains: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawFeed {
    id: String,
    url: String,
    base_poll_seconds: Option<u64>,
}

pub struct ConfigLoader;

pub struct LoadedConfig {
    pub app: AppConfig,
    pub feeds: Vec<FeedConfig>,
    pub categories: Vec<CategoryConfig>,
}

impl ConfigLoader {
    pub async fn load(config_path: &Path) -> Result<LoadedConfig, ConfigError> {
        let default_timezone = "America/Mexico_City";

        let base_dir = config_path
            .parent()
            .ok_or_else(|| ConfigError::Invalid("config path has no parent".into()))?;
        let domains_path = base_dir.join("domains.toml");
        let categories_path = base_dir.join("categories.toml");
        let feeds_dir = base_dir.join("feeds");

        let app_content = fs::read_to_string(config_path).await?;
        let raw_cfg: RawAppFile = toml::from_str(&app_content)?;

        let domains_content = fs::read_to_string(&domains_path).await?;
        let raw_domains: RawDomainsFile = toml::from_str(&domains_content)?;

        let categories_content = fs::read_to_string(&categories_path).await?;
        let raw_categories: RawCategoriesFile = toml::from_str(&categories_content)?;

        let raw_feeds = Self::load_all_feeds(&feeds_dir).await?;

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

        let db_base = resolve_db_base_dir(config_path);
        let db_path = db_base.join(raw_cfg.sqlite.path);
        let db_dialect = parse_dialect(raw_cfg.database.dialect.as_deref())?;
        let postgres = parse_postgres(raw_cfg.postgres)?;

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

        // Derive feed domain from URL host, like Scala does. :contentReference[oaicite:2]{index=2}
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
                jitter_fraction: raw_cfg.polling.jitter_fraction,
                global_max_concurrent_requests: raw_cfg.requests.global_max_concurrent_requests,
                user_agent: raw_cfg.requests.user_agent,
                log_level,
                log_file_level,
                log_file_directory: log_dir,
                log_file_name: raw_cfg.logging.file_name,
                log_file_rotation,
                mode,
                timezone,
                domains,
                state_history_sample_rate: history_sample_rate,
            },
            feeds,
            categories,
        })
    }

    async fn load_all_feeds(feeds_dir: &Path) -> Result<RawFeedsFile, ConfigError> {
        let files = collect_feed_files(feeds_dir).await?;

        if files.is_empty() {
            return Err(ConfigError::Invalid(format!(
                "no feed files found in {}",
                feeds_dir.display()
            )));
        }

        let mut all = Vec::new();
        for p in files {
            let content = fs::read_to_string(&p).await?;
            let parsed: RawFeedsFile = toml::from_str(&content)?;
            all.extend(parsed.feeds);
        }
        Ok(RawFeedsFile { feeds: all })
    }
}

async fn collect_feed_files(feeds_dir: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let mut entries = fs::read_dir(feeds_dir).await.map_err(|_| {
        ConfigError::Invalid(format!("feeds dir not found at {}", feeds_dir.display()))
    })?;

    let mut files: Vec<PathBuf> = Vec::new();
    while let Some(e) = entries.next_entry().await? {
        let p = e.path();
        let ty = e.file_type().await?;
        if ty.is_file() && is_toml_file(&p) {
            files.push(p);
        } else if ty.is_dir() {
            let mut sub_entries = fs::read_dir(&p).await?;
            while let Some(sub) = sub_entries.next_entry().await? {
                let sub_path = sub.path();
                let sub_ty = sub.file_type().await?;
                if sub_ty.is_file() && is_toml_file(&sub_path) {
                    files.push(sub_path);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

fn is_toml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
}

fn parse_dialect(s: Option<&str>) -> Result<SqlDialect, ConfigError> {
    match s.map(|x| x.to_ascii_lowercase()) {
        None => Ok(SqlDialect::Sqlite),
        Some(d) if d == "sqlite" => Ok(SqlDialect::Sqlite),
        Some(d) if d == "postgres" => Ok(SqlDialect::Postgres),
        Some(other) => Err(ConfigError::Invalid(format!(
            "invalid database.dialect '{other}', expected 'sqlite' or 'postgres'"
        ))),
    }
}

fn parse_postgres(raw: Option<RawPostgres>) -> Result<PostgresConfig, ConfigError> {
    let pg = raw.unwrap_or_default();
    Ok(PostgresConfig {
        user: pg.user,
        password: pg.password,
        host: pg.host,
        port: pg.port,
        database: pg.db,
    })
}

fn default_pg_user() -> String {
    "admin".to_string()
}

fn default_pg_password() -> String {
    "admin".to_string()
}

fn default_pg_host() -> String {
    "localhost".to_string()
}

fn default_pg_port() -> u16 {
    5432
}

fn default_pg_database() -> String {
    "data".to_string()
}

fn default_sqlite_path() -> String {
    "rss.db".to_string()
}

fn default_log_file_directory() -> String {
    "logs".to_string()
}

fn default_log_file_name() -> String {
    "feedrv3".to_string()
}

fn default_log_file_rotation() -> String {
    "hourly".to_string()
}

fn default_log_file_level() -> String {
    "info".to_string()
}

fn default_max_consecutive_errors() -> u32 {
    5
}

fn parse_mode(s: Option<&str>) -> Result<AppMode, ConfigError> {
    match s.map(|x| x.to_ascii_lowercase()) {
        None => Ok(AppMode::Prod),
        Some(m) if m == "prod" => Ok(AppMode::Prod),
        Some(m) if m == "dev" => Ok(AppMode::Dev),
        Some(other) => Err(ConfigError::Invalid(format!(
            "invalid app.mode '{other}', expected 'dev' or 'prod'"
        ))),
    }
}

fn normalize_log_level(level: &str) -> Result<String, ConfigError> {
    let l = level.trim().to_ascii_lowercase();
    if l.is_empty() {
        return Err(ConfigError::Invalid("logging.file_level cannot be empty".into()));
    }
    match l.as_str() {
        "error" | "warn" | "info" | "debug" | "trace" | "off" => Ok(l),
        _ => Err(ConfigError::Invalid(format!(
            "invalid logging.file_level '{level}', expected error|warn|info|debug|trace|off"
        ))),
    }
}

fn normalize_log_rotation(rotation: &str) -> Result<String, ConfigError> {
    let r = rotation.trim().to_ascii_lowercase();
    if r.is_empty() {
        return Err(ConfigError::Invalid(
            "logging.file_rotation cannot be empty".into(),
        ));
    }
    match r.as_str() {
        "hourly" => Ok(r),
        _ => Err(ConfigError::Invalid(format!(
            "invalid logging.file_rotation '{rotation}', expected 'hourly'"
        ))),
    }
}

// Mimics Scala's "if path is under resources, base is CWD else config parent". :contentReference[oaicite:3]{index=3}
fn resolve_db_base_dir(config_path: &Path) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path_str = config_path.to_string_lossy();
    if path_str.contains("resources") {
        cwd
    } else {
        config_path.parent().unwrap_or(&cwd).to_path_buf()
    }
}

fn resolve_log_dir(config_path: &Path, log_dir: &str) -> PathBuf {
    let p = Path::new(log_dir);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(p)
}

fn url_host(url: &str) -> Option<String> {
    // Minimal, dependency-free host extraction.
    // If you prefer stricter parsing, add `url = "2"` and use `Url::parse`.
    let u = url.trim();
    let after_scheme = u.split("://").nth(1)?;
    let host_port = after_scheme.split('/').next()?;
    let host = host_port.split('@').last().unwrap_or(host_port);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}
