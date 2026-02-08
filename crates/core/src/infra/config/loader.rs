use std::collections::{
  HashMap,
  HashSet
};
use std::path::Path;

use chrono_tz::Tz;
use tokio::fs;

use super::ConfigError;
use super::defaults::{
  default_metrics_bind,
  default_metrics_enabled,
  normalize_domains,
  normalize_log_level,
  normalize_log_rotation,
  normalize_status_codes
};
use super::feeds::load_all_feeds;
use super::parse::{
  parse_dialect,
  parse_mode,
  parse_postgres,
  url_host
};
use super::paths::{
  resolve_db_base_dir,
  resolve_log_dir
};
use super::raw::{
  RawAppFile,
  RawCategoriesFile,
  RawDomainsFile,
  RawMetrics,
  RawWatch
};
use super::schema::{
  load_schema,
  validate_toml
};
use crate::domain::model::{
  AppConfig,
  CategoryConfig,
  DomainConfig,
  FeedConfig,
  MetricsConfig,
  PostgresConfig,
  WatchCheckMethod,
  WatchConfig,
  WatchDetector,
  WatchEmitMode,
  WatchItemIdentity
};

pub struct ConfigLoader;

pub struct LoadedConfig {
  pub app:        AppConfig,
  pub feeds:      Vec<FeedConfig>,
  pub watches:    Vec<WatchConfig>,
  pub categories: Vec<CategoryConfig>
}

impl ConfigLoader {
  pub async fn load(
    config_path: &Path
  ) -> Result<LoadedConfig, ConfigError>
  {
    let default_timezone =
      "America/Mexico_City";

    let base_dir = config_path
      .parent()
      .ok_or_else(|| {
        ConfigError::Invalid(
          "config path has no parent"
            .into()
        )
      })?;

    let domains_path =
      base_dir.join("domains.toml");

    let categories_path =
      base_dir.join("categories.toml");

    let feeds_dir = match std::env::var(
      "FEEDS_DIR"
    ) {
      | Ok(p) if !p.trim().is_empty() => {
        Path::new(p.trim())
          .to_path_buf()
      }
      | _ => base_dir.join("feeds")
    };

    let schema_dir =
      base_dir.join("schemas");

    let config_schema = load_schema(
      &schema_dir,
      "config.schema.json"
    )
    .await?;

    let domains_schema = load_schema(
      &schema_dir,
      "domains.schema.json"
    )
    .await?;

    let categories_schema =
      load_schema(
        &schema_dir,
        "categories.schema.json"
      )
      .await?;

    let feeds_schema = load_schema(
      &schema_dir,
      "feeds.schema.json"
    )
    .await?;

    let global_schema = load_schema(
      &schema_dir,
      "global.schema.json"
    )
    .await?;

    let app_content =
      fs::read_to_string(config_path)
        .await?;

    validate_toml(
      &config_schema,
      &app_content,
      &config_path
        .display()
        .to_string()
    )?;

    let raw_cfg: RawAppFile =
      toml::from_str(&app_content)?;

    let domains_content =
      fs::read_to_string(&domains_path)
        .await?;

    validate_toml(
      &domains_schema,
      &domains_content,
      &domains_path
        .display()
        .to_string()
    )?;

    let raw_domains: RawDomainsFile =
      toml::from_str(&domains_content)?;

    let categories_content =
      fs::read_to_string(
        &categories_path
      )
      .await?;

    validate_toml(
      &categories_schema,
      &categories_content,
      &categories_path
        .display()
        .to_string()
    )?;

    let raw_categories: RawCategoriesFile =
      toml::from_str(&categories_content)?;

    let raw_feeds = load_all_feeds(
      &feeds_dir,
      &feeds_schema,
      &global_schema
    )
    .await?;

    let mode = parse_mode(
      raw_cfg.app.mode.as_deref()
    )?;

    let tz_str = raw_cfg
      .app
      .timezone
      .as_deref()
      .filter(|s| !s.trim().is_empty())
      .unwrap_or(default_timezone);

    let timezone: Tz =
      tz_str.parse().map_err(|_| {
        ConfigError::Invalid(format!(
          "invalid timezone '{tz_str}'"
        ))
      })?;

    let log_level = raw_cfg
      .logging
      .level
      .clone()
      .unwrap_or_else(|| {
        "info".to_string()
      });

    let log_file_level =
      normalize_log_level(
        &raw_cfg.logging.file_level
      )?;

    let log_file_rotation =
      normalize_log_rotation(
        &raw_cfg.logging.file_rotation
      )?;

    let log_dir = resolve_log_dir(
      config_path,
      &raw_cfg.logging.file_directory
    );

    let feed_timing_domains =
      normalize_domains(
        &raw_cfg
          .logging
          .feed_timing_domains
      )?;

    let immediate_error_statuses =
      normalize_status_codes(
        &raw_cfg
          .backoff
          .immediate_error_statuses
      )?;

    let db_base =
      resolve_db_base_dir(config_path);

    let db_path =
      db_base.join(raw_cfg.sqlite.path);

    let db_dialect = parse_dialect(
      raw_cfg
        .database
        .dialect
        .as_deref()
    )?;

    let postgres: PostgresConfig =
      parse_postgres(raw_cfg.postgres)?;

    let mut domains = HashMap::new();

    for d in raw_domains.domains {
      domains.insert(
        d.name,
        DomainConfig {
          max_concurrent_requests: d
            .max_concurrent_requests
        }
      );
    }

    let mut category_names =
      HashSet::new();

    let mut domain_to_category =
      HashMap::new();

    let mut categories = Vec::new();

    for c in raw_categories.categories {
      let name =
        c.name.trim().to_string();

      if name.is_empty() {
        return Err(
          ConfigError::Invalid(
            "category name cannot be \
             empty"
              .into()
          )
        );
      }

      if !category_names
        .insert(name.clone())
      {
        return Err(
          ConfigError::Invalid(
            format!(
              "duplicate category \
               name '{name}'"
            )
          )
        );
      }

      let mut domains_vec = Vec::new();

      for d in c.domains {
        let domain =
          d.trim().to_ascii_lowercase();

        if domain.is_empty() {
          return Err(
            ConfigError::Invalid(
              format!(
                "category '{name}' \
                 has empty domain"
              )
            )
          );
        }

        if domain_to_category
          .insert(
            domain.clone(),
            name.clone()
          )
          .is_some()
        {
          return Err(
            ConfigError::Invalid(
              format!(
                "domain '{domain}' \
                 appears in multiple \
                 categories"
              )
            )
          );
        }

        domains_vec.push(domain);
      }

      categories.push(CategoryConfig {
        name,
        domains: domains_vec
      });
    }

    if categories.is_empty() {
      return Err(ConfigError::Invalid(
        "categories.toml must define \
         at least one category"
          .into()
      ));
    }

    let history_sample_rate = raw_cfg
      .state_history
      .as_ref()
      .and_then(|s| s.sample_rate)
      .unwrap_or(1.0);

    if !(0.0..=1.0)
      .contains(&history_sample_rate)
    {
      return Err(ConfigError::Invalid(
        format!(
          "state_history.sample_rate \
           must be between 0 and 1, \
           got {history_sample_rate}"
        )
      ));
    }

    let mut feeds = Vec::new();
    let mut watches = Vec::new();
    let mut source_ids = HashSet::new();

    for f in raw_feeds.feeds {
      let domain = url_host(&f.url)
        .ok_or_else(|| {
          ConfigError::Invalid(format!(
            "feed '{}' missing host",
            f.id,
          ))
        })?
        .to_ascii_lowercase();

      let category = resolve_category(
        &f.id,
        "feed",
        &f.category,
        &domain,
        &category_names,
        &domain_to_category
      )?;

      if !source_ids
        .insert(f.id.clone())
      {
        return Err(
          ConfigError::Invalid(
            format!(
              "duplicate source id \
               '{}'",
              f.id
            )
          )
        );
      }

      feeds.push(FeedConfig {
        id: f.id,
        url: f.url,
        domain,
        category,
        base_poll_seconds: f
          .base_poll_seconds
          .unwrap_or(
            raw_cfg
              .polling
              .default_seconds
          ),
        provenance: f.provenance,
        tags: f.tags,
        language: f.language,
        content_type: f.content_type,
        cookie_path:
          normalize_optional_string(
            f.cookie_path
          )
      });
    }

    for w in raw_feeds.watches {
      let watch = parse_watch(
        w,
        raw_cfg.polling.default_seconds,
        &category_names,
        &domain_to_category
      )?;

      if !source_ids
        .insert(watch.id.clone())
      {
        return Err(
          ConfigError::Invalid(
            format!(
              "duplicate source id \
               '{}'",
              watch.id
            )
          )
        );
      }

      watches.push(watch);
    }

    let metrics_cfg = raw_cfg
      .metrics
      .unwrap_or(RawMetrics {
        enabled:
          default_metrics_enabled(),
        bind:    default_metrics_bind()
      });

    Ok(LoadedConfig {
      app: AppConfig {
        db_dialect,
        sqlite_path: db_path,
        postgres,
        default_poll_seconds: raw_cfg
          .polling
          .default_seconds,
        max_poll_seconds:
          raw_cfg.polling.max_seconds,
        error_backoff_base_seconds: raw_cfg
          .backoff
          .error_base_seconds,
        max_error_backoff_seconds: raw_cfg
          .backoff
          .max_error_seconds,
        max_consecutive_errors: raw_cfg
          .backoff
          .max_consecutive_errors,
        immediate_error_statuses,
        jitter_fraction: raw_cfg
          .polling
          .jitter_fraction,
        global_max_concurrent_requests:
          raw_cfg
            .requests
            .global_max_concurrent_requests,
        user_agent:
          raw_cfg.requests.user_agent,
        log_level,
        log_file_enabled: raw_cfg
          .logging
          .file_enabled,
        log_file_level,
        log_file_directory: log_dir,
        log_file_name:
          raw_cfg.logging.file_name,
        log_file_rotation,
        log_tick_warn_seconds: raw_cfg
          .logging
          .tick_warn_seconds,
        log_feed_timing_enabled: raw_cfg
          .logging
          .feed_timing_enabled,
        log_feed_timing_domains:
          feed_timing_domains,
        log_feed_timing_warn_ms: raw_cfg
          .logging
          .feed_timing_warn_ms,
        log_feed_timing_log_all: raw_cfg
          .logging
          .feed_timing_log_all,
        metrics: MetricsConfig {
          enabled:
            metrics_cfg.enabled,
          bind: metrics_cfg.bind,
        },
        mode,
        timezone,
        domains,
        state_history_sample_rate: history_sample_rate,
      },
      feeds,
      watches,
      categories,
    })
  }
}

fn resolve_category(
  source_id: &str,
  source_kind: &str,
  configured_category: &Option<String>,
  domain: &str,
  category_names: &HashSet<String>,
  domain_to_category: &HashMap<
    String,
    String
  >
) -> Result<String, ConfigError> {
  if let Some(cat) =
    configured_category.clone()
  {
    if !category_names.contains(&cat) {
      return Err(ConfigError::Invalid(
        format!(
          "{source_kind} \
           '{source_id}' category \
           '{cat}' missing from \
           categories"
        )
      ));
    }

    return Ok(cat);
  }

  domain_to_category
    .get(domain)
    .cloned()
    .ok_or_else(|| {
      ConfigError::Invalid(format!(
        "{source_kind} '{source_id}' \
         domain '{domain}' missing \
         from categories"
      ))
    })
}

fn parse_watch(
  w: RawWatch,
  default_poll_seconds: u64,
  category_names: &HashSet<String>,
  domain_to_category: &HashMap<
    String,
    String
  >
) -> Result<WatchConfig, ConfigError> {
  let domain = url_host(&w.url)
    .ok_or_else(|| {
      ConfigError::Invalid(format!(
        "watch '{}' missing host",
        w.id
      ))
    })?
    .to_ascii_lowercase();

  let category = resolve_category(
    &w.id,
    "watch",
    &w.category,
    &domain,
    category_names,
    domain_to_category
  )?;

  let check_method =
    parse_check_method(
      w.check_method.as_deref(),
      &w.id
    )?;

  let detectors = parse_detectors(
    w.detectors.as_ref(),
    &w.id
  )?;

  let emit_mode = parse_emit_mode(
    w.emit_mode.as_deref(),
    &w.id
  )?;

  let item_identity =
    parse_item_identity(
      w.item_identity.as_deref(),
      &w.id
    )?;

  if w
    .item_selector
    .as_deref()
    .map(|s| s.trim().is_empty())
    .unwrap_or(true)
  {
    return Err(ConfigError::Invalid(
      format!(
        "watch '{}' requires \
         item_selector",
        w.id
      )
    ));
  }

  if matches!(
    item_identity,
    Some(WatchItemIdentity::Attr)
  ) && w
    .item_identity_attr
    .as_deref()
    .map(|s| s.trim().is_empty())
    .unwrap_or(true)
  {
    return Err(ConfigError::Invalid(
      format!(
        "watch '{}' requires \
         item_identity_attr when \
         item_identity='attr'",
        w.id
      )
    ));
  }

  Ok(WatchConfig {
    id: w.id,
    url: w.url,
    domain,
    category,
    base_poll_seconds: w
      .base_poll_seconds
      .unwrap_or(default_poll_seconds),
    provenance: w.provenance,
    tags: w.tags,
    language: w.language,
    content_type: w.content_type,
    cookie_path:
      normalize_optional_string(
        w.cookie_path
      ),
    check_method,
    fallback_to_get: w
      .fallback_to_get
      .unwrap_or(true),
    detectors,
    fetch_body_on_change: w
      .fetch_body_on_change
      .unwrap_or(true),
    max_body_bytes: w.max_body_bytes,
    item_selector: w.item_selector,
    item_identity,
    item_identity_attr: w
      .item_identity_attr,
    title_selector: w.title_selector,
    link_selector: w.link_selector,
    summary_selector: w
      .summary_selector,
    published_selector: w
      .published_selector,
    published_format: w
      .published_format,
    include_selectors: w
      .include_selectors,
    exclude_selectors: w
      .exclude_selectors,
    normalize_whitespace: w
      .normalize_whitespace
      .unwrap_or(true),
    strip_query_params: w
      .strip_query_params
      .unwrap_or(false),
    emit_mode,
    emit_title: w.emit_title,
    min_item_count_change: w
      .min_item_count_change
  })
}

fn normalize_optional_string(
  raw: Option<String>
) -> Option<String> {
  raw.and_then(|s| {
    let trimmed = s.trim();
    if trimmed.is_empty() {
      None
    } else {
      Some(trimmed.to_string())
    }
  })
}

fn parse_check_method(
  raw: Option<&str>,
  watch_id: &str
) -> Result<WatchCheckMethod, ConfigError>
{
  match raw
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(|s| s.to_ascii_lowercase())
  {
    | None => {
      Ok(WatchCheckMethod::Head)
    }
    | Some(s) if s == "head" => {
      Ok(WatchCheckMethod::Head)
    }
    | Some(s) if s == "get" => {
      Ok(WatchCheckMethod::Get)
    }
    | Some(other) => {
      Err(ConfigError::Invalid(
        format!(
          "watch '{}' has invalid \
           check_method '{}', \
           expected 'head' or 'get'",
          watch_id, other
        )
      ))
    }
  }
}

fn parse_detectors(
  raw: Option<&Vec<String>>,
  watch_id: &str
) -> Result<
  Vec<WatchDetector>,
  ConfigError
> {
  let values: Vec<String> = match raw {
    | Some(v) if !v.is_empty() => {
      v.clone()
    }
    | _ => {
      vec![
        "etag".to_string(),
        "last_modified".to_string(),
      ]
    }
  };

  let mut parsed = Vec::new();

  for detector in values {
    let normalized = detector
      .trim()
      .to_ascii_lowercase();

    let parsed_detector =
      match normalized.as_str() {
        | "etag" => WatchDetector::Etag,
        | "last_modified" => {
          WatchDetector::LastModified
        }
        | "content_length" => {
          WatchDetector::ContentLength
        }
        | "content_hash" => {
          WatchDetector::ContentHash
        }
        | "element_hash" => {
          WatchDetector::ElementHash
        }
        | _ => {
          return Err(
            ConfigError::Invalid(
              format!(
                "watch '{}' has \
                 invalid detector \
                 '{}'; expected one \
                 of etag,last_modified,\
                 content_length,\
                 content_hash,\
                 element_hash",
                watch_id, detector
              )
            )
          );
        }
      };

    if !parsed
      .contains(&parsed_detector)
    {
      parsed.push(parsed_detector);
    }
  }

  Ok(parsed)
}

fn parse_emit_mode(
  raw: Option<&str>,
  watch_id: &str
) -> Result<WatchEmitMode, ConfigError>
{
  match raw
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(|s| s.to_ascii_lowercase())
  {
    | None => {
      Ok(WatchEmitMode::NewItemsOnly)
    }
    | Some(s)
      if s == "new_items_only" =>
    {
      Ok(WatchEmitMode::NewItemsOnly)
    }
    | Some(s) if s == "any_change" => {
      Ok(WatchEmitMode::AnyChange)
    }
    | Some(s) if s == "digest" => {
      Ok(WatchEmitMode::Digest)
    }
    | Some(other) => {
      Err(ConfigError::Invalid(
        format!(
          "watch '{}' has invalid \
           emit_mode '{}', expected \
           'new_items_only', \
           'any_change', or 'digest'",
          watch_id, other
        )
      ))
    }
  }
}

fn parse_item_identity(
  raw: Option<&str>,
  watch_id: &str
) -> Result<
  Option<WatchItemIdentity>,
  ConfigError
> {
  match raw
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(|s| s.to_ascii_lowercase())
  {
    | None => Ok(None),
    | Some(s) if s == "href" => {
      Ok(Some(WatchItemIdentity::Href))
    }
    | Some(s) if s == "text" => {
      Ok(Some(WatchItemIdentity::Text))
    }
    | Some(s) if s == "attr" => {
      Ok(Some(WatchItemIdentity::Attr))
    }
    | Some(other) => {
      Err(ConfigError::Invalid(
        format!(
          "watch '{}' has invalid \
           item_identity '{}', \
           expected 'href', 'text', \
           or 'attr'",
          watch_id, other
        )
      ))
    }
  }
}
