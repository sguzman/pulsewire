use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::fs;

use super::raw::RawFeedDefaults;
use super::raw::RawFeedsFile;
use super::schema::validate_toml;
use super::ConfigError;

pub(crate) async fn load_all_feeds(
    feeds_dir: &Path,
    feeds_schema: &str,
    global_schema: &str,
) -> Result<RawFeedsFile, ConfigError> {
    let files = collect_feed_files(feeds_dir).await?;

    if files.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "no feed files found in {}",
            feeds_dir.display()
        )));
    }

    let mut all = Vec::new();
    let mut global_cache: HashMap<PathBuf, FeedDefaults> = HashMap::new();
    for p in files {
        let dir = p
            .parent()
            .ok_or_else(|| ConfigError::Invalid(format!("feed file missing parent: {}", p.display())))?
            .to_path_buf();
        let global_defaults = load_global_defaults(&dir, global_schema, &mut global_cache).await?;

        let content = fs::read_to_string(&p).await?;
        validate_toml(feeds_schema, &content, &p.display().to_string())?;
        let parsed: RawFeedsFile = toml::from_str(&content)?;
        let file_defaults = FeedDefaults::from_file(&parsed, &p)?;
        let defaults = FeedDefaults::merge(&global_defaults, &file_defaults);
        for feed in parsed.feeds {
            all.push(apply_defaults(feed, &defaults, &p)?);
        }
    }
    Ok(RawFeedsFile {
        base_poll_seconds: None,
        id_prefix: None,
        category: None,
        provenance: None,
        tags: None,
        language: None,
        content_type: None,
        feeds: all,
    })
}

async fn collect_feed_files(feeds_dir: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let mut entries = fs::read_dir(feeds_dir).await.map_err(|_| {
        ConfigError::Invalid(format!("feeds dir not found at {}", feeds_dir.display()))
    })?;

    let mut files: Vec<PathBuf> = Vec::new();
    while let Some(e) = entries.next_entry().await? {
        let p = e.path();
        let ty = e.file_type().await?;
        if ty.is_file() && is_toml_file(&p) && !is_global_file(&p) {
            files.push(p);
        } else if ty.is_dir() {
            let mut sub_entries = fs::read_dir(&p).await?;
            while let Some(sub) = sub_entries.next_entry().await? {
                let sub_path = sub.path();
                let sub_ty = sub.file_type().await?;
                if sub_ty.is_file() && is_toml_file(&sub_path) && !is_global_file(&sub_path) {
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

fn is_global_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("global.toml"))
        .unwrap_or(false)
}

async fn load_global_defaults(
    dir: &Path,
    global_schema: &str,
    cache: &mut HashMap<PathBuf, FeedDefaults>,
) -> Result<FeedDefaults, ConfigError> {
    if let Some(defaults) = cache.get(dir) {
        return Ok(defaults.clone());
    }

    let global_path = dir.join("global.toml");
    let defaults = match fs::read_to_string(&global_path).await {
        Ok(content) => {
            validate_toml(global_schema, &content, &global_path.display().to_string())?;
            let parsed: RawFeedDefaults = toml::from_str(&content)?;
            FeedDefaults::from_defaults_file(&parsed, &global_path)?
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => FeedDefaults::empty(),
        Err(err) => return Err(err.into()),
    };

    cache.insert(dir.to_path_buf(), defaults.clone());
    Ok(defaults)
}

#[derive(Clone)]
struct FeedDefaults {
    base_poll_seconds: Option<u64>,
    id_prefix: Option<String>,
    category: Option<String>,
    provenance: Option<String>,
    tags: Option<Vec<String>>,
    language: Option<String>,
    content_type: Option<String>,
}

impl FeedDefaults {
    fn empty() -> Self {
        Self {
            base_poll_seconds: None,
            id_prefix: None,
            category: None,
            provenance: None,
            tags: None,
            language: None,
            content_type: None,
        }
    }

    fn from_file(file: &RawFeedsFile, path: &Path) -> Result<Self, ConfigError> {
        let id_prefix = match file.id_prefix.as_deref() {
            Some(prefix) => Some(validate_id_prefix(
                prefix,
                &format!("feed defaults in {}", path.display()),
            )?),
            None => None,
        };

        Ok(Self {
            base_poll_seconds: file.base_poll_seconds,
            id_prefix,
            category: file.category.clone(),
            provenance: file.provenance.clone(),
            tags: file.tags.clone(),
            language: file.language.clone(),
            content_type: file.content_type.clone(),
        })
    }

    fn from_defaults_file(file: &RawFeedDefaults, path: &Path) -> Result<Self, ConfigError> {
        let id_prefix = match file.id_prefix.as_deref() {
            Some(prefix) => Some(validate_id_prefix(
                prefix,
                &format!("global defaults in {}", path.display()),
            )?),
            None => None,
        };

        Ok(Self {
            base_poll_seconds: file.base_poll_seconds,
            id_prefix,
            category: file.category.clone(),
            provenance: file.provenance.clone(),
            tags: file.tags.clone(),
            language: file.language.clone(),
            content_type: file.content_type.clone(),
        })
    }

    fn merge(global: &Self, file: &Self) -> Self {
        Self {
            base_poll_seconds: file.base_poll_seconds.or(global.base_poll_seconds),
            id_prefix: file.id_prefix.clone().or_else(|| global.id_prefix.clone()),
            category: file.category.clone().or_else(|| global.category.clone()),
            provenance: file.provenance.clone().or_else(|| global.provenance.clone()),
            tags: file.tags.clone().or_else(|| global.tags.clone()),
            language: file.language.clone().or_else(|| global.language.clone()),
            content_type: file.content_type.clone().or_else(|| global.content_type.clone()),
        }
    }
}

fn apply_defaults(
    mut feed: super::raw::RawFeed,
    defaults: &FeedDefaults,
    path: &Path,
) -> Result<super::raw::RawFeed, ConfigError> {
    if feed.base_poll_seconds.is_none() {
        feed.base_poll_seconds = defaults.base_poll_seconds;
    }
    if feed.category.is_none() {
        feed.category = defaults.category.clone();
    }
    if feed.provenance.is_none() {
        feed.provenance = defaults.provenance.clone();
    }
    if feed.tags.is_none() {
        feed.tags = defaults.tags.clone();
    }
    if feed.language.is_none() {
        feed.language = defaults.language.clone();
    }
    if feed.content_type.is_none() {
        feed.content_type = defaults.content_type.clone();
    }

    let prefix = match feed.id_prefix.as_deref() {
        Some(raw) => {
            let normalized = validate_id_prefix(
                raw,
                &format!("feed '{}' in {}", feed.id, path.display()),
            )?;
            feed.id_prefix = Some(normalized.clone());
            Some(normalized)
        }
        None => defaults.id_prefix.clone(),
    };

    if let Some(prefix) = prefix {
        feed.id = format!("{prefix}-{}", feed.id);
    }

    Ok(feed)
}

fn validate_id_prefix(prefix: &str, context: &str) -> Result<String, ConfigError> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "{context} id_prefix cannot be empty"
        )));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err(ConfigError::Invalid(format!(
            "{context} id_prefix must be lowercase alphanumeric"
        )));
    }
    Ok(trimmed.to_string())
}
