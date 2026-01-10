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
        all.extend(parsed.feeds);
    }
    Ok(RawFeedsFile { feeds: all })
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
