use std::path::{Path, PathBuf};

use tokio::fs;

use super::raw::RawFeedsFile;
use super::schema::validate_toml;
use super::ConfigError;

pub(crate) async fn load_all_feeds(
    feeds_dir: &Path,
    feeds_schema: &str,
) -> Result<RawFeedsFile, ConfigError> {
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
        validate_toml(feeds_schema, &content, &p.display().to_string())?;
        let parsed: RawFeedsFile = toml::from_str(&content)?;
        let defaults = FeedDefaults::from_file(&parsed, &p)?;
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
