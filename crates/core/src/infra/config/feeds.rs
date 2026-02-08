use std::collections::HashMap;
use std::path::{
  Path,
  PathBuf
};

use tokio::fs;

use super::ConfigError;
use super::raw::{
  RawFeedDefaults,
  RawFeedsFile,
  RawWatch,
  RawWatchDefaults
};
use super::schema::validate_toml;

pub(crate) async fn load_all_feeds(
  feeds_dir: &Path,
  feeds_schema: &str,
  global_schema: &str
) -> Result<RawFeedsFile, ConfigError> {
  let files =
    collect_feed_files(feeds_dir)
      .await?;

  if files.is_empty() {
    return Err(ConfigError::Invalid(
      format!(
        "no feed files found in {}",
        feeds_dir.display()
      )
    ));
  }

  let mut all_feeds = Vec::new();
  let mut all_watches = Vec::new();

  let mut global_cache: HashMap<
    PathBuf,
    MergedDefaults
  > = HashMap::new();

  for p in files {
    let dir = p
      .parent()
      .ok_or_else(|| {
        ConfigError::Invalid(format!(
          "feed file missing parent: \
           {}",
          p.display()
        ))
      })?
      .to_path_buf();

    let merged_global =
      load_global_defaults(
        &dir,
        global_schema,
        &mut global_cache
      )
      .await?;

    let content =
      fs::read_to_string(&p).await?;

    validate_toml(
      feeds_schema,
      &content,
      &p.display().to_string()
    )?;

    let parsed: RawFeedsFile =
      toml::from_str(&content)?;

    let file_feed_defaults =
      FeedDefaults::from_file(
        &parsed, &p
      )?;
    let feed_defaults =
      FeedDefaults::merge(
        &merged_global.feed,
        &file_feed_defaults
      );

    for feed in parsed.feeds {
      all_feeds.push(
        apply_feed_defaults(
          feed,
          &feed_defaults,
          &p
        )?
      );
    }

    let file_watch_defaults =
      WatchDefaults::from_optional(
        parsed.watch_defaults.as_ref(),
        &p,
        "watch defaults"
      )?;

    let watch_defaults =
      WatchDefaults::merge(
        &merged_global.watch,
        &file_watch_defaults
      );

    let mut profiles = HashMap::new();
    for profile in parsed.watch_profiles
    {
      let name =
        profile.name.trim().to_string();

      if name.is_empty() {
        return Err(
          ConfigError::Invalid(
            format!(
              "watch profile name \
               cannot be empty in {}",
              p.display()
            )
          )
        );
      }

      if profiles.contains_key(&name) {
        return Err(
          ConfigError::Invalid(
            format!(
              "duplicate watch \
               profile '{}' in {}",
              name,
              p.display()
            )
          )
        );
      }

      let raw_profile =
        WatchDefaults::from_optional(
          Some(&profile.defaults),
          &p,
          &format!(
            "watch profile '{}'",
            name
          )
        )?;

      profiles.insert(
        name,
        WatchDefaults::merge(
          &watch_defaults,
          &raw_profile
        )
      );
    }

    for watch in parsed.watches {
      all_watches.push(
        apply_watch_defaults(
          watch,
          &watch_defaults,
          &profiles,
          &p
        )?
      );
    }
  }

  Ok(RawFeedsFile {
    base_poll_seconds: None,
    id_prefix:         None,
    category:          None,
    provenance:        None,
    tags:              None,
    language:          None,
    content_type:      None,
    cookie_path:       None,
    feeds:             all_feeds,
    watch_defaults:    None,
    watch_profiles:    Vec::new(),
    watches:           all_watches
  })
}

async fn collect_feed_files(
  feeds_dir: &Path
) -> Result<Vec<PathBuf>, ConfigError> {
  let mut entries =
    fs::read_dir(feeds_dir)
      .await
      .map_err(|_| {
        ConfigError::Invalid(format!(
          "feeds dir not found at {}",
          feeds_dir.display()
        ))
      })?;

  let mut files: Vec<PathBuf> =
    Vec::new();

  while let Some(e) =
    entries.next_entry().await?
  {
    let p = e.path();

    let ty = e.file_type().await?;

    if ty.is_file()
      && is_toml_file(&p)
      && !is_global_file(&p)
    {
      files.push(p);
    } else if ty.is_dir() {
      let mut sub_entries =
        fs::read_dir(&p).await?;

      while let Some(sub) =
        sub_entries.next_entry().await?
      {
        let sub_path = sub.path();

        let sub_ty =
          sub.file_type().await?;

        if sub_ty.is_file()
          && is_toml_file(&sub_path)
          && !is_global_file(&sub_path)
        {
          files.push(sub_path);
        }
      }
    }
  }

  files.sort();

  Ok(files)
}

fn is_toml_file(path: &Path) -> bool {
  path
    .extension()
    .and_then(|s| s.to_str())
    .map(|s| {
      s.eq_ignore_ascii_case("toml")
    })
    .unwrap_or(false)
}

fn is_global_file(path: &Path) -> bool {
  path
    .file_name()
    .and_then(|s| s.to_str())
    .map(|s| {
      s.eq_ignore_ascii_case(
        "global.toml"
      )
    })
    .unwrap_or(false)
}

#[derive(Clone)]
struct MergedDefaults {
  feed:  FeedDefaults,
  watch: WatchDefaults
}

async fn load_global_defaults(
  dir: &Path,
  global_schema: &str,
  cache: &mut HashMap<
    PathBuf,
    MergedDefaults
  >
) -> Result<MergedDefaults, ConfigError>
{
  if let Some(defaults) = cache.get(dir)
  {
    return Ok(defaults.clone());
  }

  let global_path =
    dir.join("global.toml");

  let defaults = match fs::read_to_string(&global_path).await {
        Ok(content) => {
            validate_toml(global_schema, &content, &global_path.display().to_string())?;
            let parsed: RawFeedDefaults = toml::from_str(&content)?;
            MergedDefaults {
              feed: FeedDefaults::from_defaults_file(&parsed, &global_path)?,
              watch: WatchDefaults::from_optional(parsed.watch_defaults.as_ref(), &global_path, "global watch defaults")?,
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => MergedDefaults {
          feed: FeedDefaults::empty(),
          watch: WatchDefaults::empty(),
        },
        Err(err) => return Err(err.into()),
    };

  cache.insert(
    dir.to_path_buf(),
    defaults.clone()
  );

  Ok(defaults)
}

#[derive(Clone)]
struct FeedDefaults {
  base_poll_seconds: Option<u64>,
  id_prefix:         Option<String>,
  category:          Option<String>,
  provenance:        Option<String>,
  tags: Option<Vec<String>>,
  language:          Option<String>,
  content_type:      Option<String>,
  cookie_path:       Option<String>
}

impl FeedDefaults {
  fn empty() -> Self {
    Self {
      base_poll_seconds: None,
      id_prefix:         None,
      category:          None,
      provenance:        None,
      tags:              None,
      language:          None,
      content_type:      None,
      cookie_path:       None
    }
  }

  fn from_file(
    file: &RawFeedsFile,
    path: &Path
  ) -> Result<Self, ConfigError> {
    let id_prefix =
      match file.id_prefix.as_deref() {
        | Some(prefix) => {
          Some(validate_id_prefix(
            prefix,
            &format!(
              "feed defaults in {}",
              path.display()
            )
          )?)
        }
        | None => None
      };

    Ok(Self {
      base_poll_seconds: file
        .base_poll_seconds,
      id_prefix,
      category: file.category.clone(),
      provenance: file
        .provenance
        .clone(),
      tags: file.tags.clone(),
      language: file.language.clone(),
      content_type: file
        .content_type
        .clone(),
      cookie_path: file
        .cookie_path
        .clone()
    })
  }

  fn from_defaults_file(
    file: &RawFeedDefaults,
    path: &Path
  ) -> Result<Self, ConfigError> {
    let id_prefix =
      match file.id_prefix.as_deref() {
        | Some(prefix) => {
          Some(validate_id_prefix(
            prefix,
            &format!(
              "global defaults in {}",
              path.display()
            )
          )?)
        }
        | None => None
      };

    Ok(Self {
      base_poll_seconds: file
        .base_poll_seconds,
      id_prefix,
      category: file.category.clone(),
      provenance: file
        .provenance
        .clone(),
      tags: file.tags.clone(),
      language: file.language.clone(),
      content_type: file
        .content_type
        .clone(),
      cookie_path: file
        .cookie_path
        .clone()
    })
  }

  fn merge(
    global: &Self,
    file: &Self
  ) -> Self {
    Self {
      base_poll_seconds: file
        .base_poll_seconds
        .or(global.base_poll_seconds),
      id_prefix:         file
        .id_prefix
        .clone()
        .or_else(|| {
          global.id_prefix.clone()
        }),
      category:          file
        .category
        .clone()
        .or_else(|| {
          global.category.clone()
        }),
      provenance:        file
        .provenance
        .clone()
        .or_else(|| {
          global.provenance.clone()
        }),
      tags:              file
        .tags
        .clone()
        .or_else(|| {
          global.tags.clone()
        }),
      language:          file
        .language
        .clone()
        .or_else(|| {
          global.language.clone()
        }),
      content_type:      file
        .content_type
        .clone()
        .or_else(|| {
          global.content_type.clone()
        }),
      cookie_path:       file
        .cookie_path
        .clone()
        .or_else(|| {
          global.cookie_path.clone()
        })
    }
  }
}

#[derive(Clone)]
struct WatchDefaults {
  base_poll_seconds:     Option<u64>,
  id_prefix:             Option<String>,
  category:              Option<String>,
  provenance:            Option<String>,
  tags: Option<Vec<String>>,
  language:              Option<String>,
  content_type:          Option<String>,
  cookie_path:           Option<String>,
  check_method:          Option<String>,
  fallback_to_get:       Option<bool>,
  detectors: Option<Vec<String>>,
  fetch_body_on_change:  Option<bool>,
  max_body_bytes:        Option<u64>,
  item_selector:         Option<String>,
  item_identity:         Option<String>,
  item_identity_attr:    Option<String>,
  title_selector:        Option<String>,
  link_selector:         Option<String>,
  summary_selector:      Option<String>,
  published_selector:    Option<String>,
  published_format:      Option<String>,
  include_selectors:
    Option<Vec<String>>,
  exclude_selectors:
    Option<Vec<String>>,
  normalize_whitespace:  Option<bool>,
  strip_query_params:    Option<bool>,
  emit_mode:             Option<String>,
  emit_title:            Option<String>,
  min_item_count_change: Option<u64>
}

impl WatchDefaults {
  fn empty() -> Self {
    Self {
      base_poll_seconds:     None,
      id_prefix:             None,
      category:              None,
      provenance:            None,
      tags:                  None,
      language:              None,
      content_type:          None,
      cookie_path:           None,
      check_method:          None,
      fallback_to_get:       None,
      detectors:             None,
      fetch_body_on_change:  None,
      max_body_bytes:        None,
      item_selector:         None,
      item_identity:         None,
      item_identity_attr:    None,
      title_selector:        None,
      link_selector:         None,
      summary_selector:      None,
      published_selector:    None,
      published_format:      None,
      include_selectors:     None,
      exclude_selectors:     None,
      normalize_whitespace:  None,
      strip_query_params:    None,
      emit_mode:             None,
      emit_title:            None,
      min_item_count_change: None
    }
  }

  fn from_optional(
    raw: Option<&RawWatchDefaults>,
    path: &Path,
    label: &str
  ) -> Result<Self, ConfigError> {
    let Some(raw) = raw else {
      return Ok(Self::empty());
    };

    let id_prefix =
      match raw.id_prefix.as_deref() {
        | Some(prefix) => {
          Some(validate_id_prefix(
            prefix,
            &format!(
              "{label} in {}",
              path.display()
            )
          )?)
        }
        | None => None
      };

    Ok(Self {
      base_poll_seconds: raw
        .base_poll_seconds,
      id_prefix,
      category: raw.category.clone(),
      provenance: raw
        .provenance
        .clone(),
      tags: raw.tags.clone(),
      language: raw.language.clone(),
      content_type: raw
        .content_type
        .clone(),
      cookie_path: raw
        .cookie_path
        .clone(),
      check_method: raw
        .check_method
        .clone(),
      fallback_to_get: raw
        .fallback_to_get,
      detectors: raw.detectors.clone(),
      fetch_body_on_change: raw
        .fetch_body_on_change,
      max_body_bytes: raw
        .max_body_bytes,
      item_selector: raw
        .item_selector
        .clone(),
      item_identity: raw
        .item_identity
        .clone(),
      item_identity_attr: raw
        .item_identity_attr
        .clone(),
      title_selector: raw
        .title_selector
        .clone(),
      link_selector: raw
        .link_selector
        .clone(),
      summary_selector: raw
        .summary_selector
        .clone(),
      published_selector: raw
        .published_selector
        .clone(),
      published_format: raw
        .published_format
        .clone(),
      include_selectors: raw
        .include_selectors
        .clone(),
      exclude_selectors: raw
        .exclude_selectors
        .clone(),
      normalize_whitespace: raw
        .normalize_whitespace,
      strip_query_params: raw
        .strip_query_params,
      emit_mode: raw.emit_mode.clone(),
      emit_title: raw
        .emit_title
        .clone(),
      min_item_count_change: raw
        .min_item_count_change
    })
  }

  fn merge(
    base: &Self,
    override_with: &Self
  ) -> Self {
    Self {
      base_poll_seconds:
        override_with
          .base_poll_seconds
          .or(base.base_poll_seconds),
      id_prefix:
        override_with
          .id_prefix
          .clone()
          .or_else(|| {
            base.id_prefix.clone()
          }),
      category:
        override_with
          .category
          .clone()
          .or_else(|| {
            base.category.clone()
          }),
      provenance:
        override_with
          .provenance
          .clone()
          .or_else(|| {
            base.provenance.clone()
          }),
      tags:
        override_with
          .tags
          .clone()
          .or_else(|| base.tags.clone()),
      language:
        override_with
          .language
          .clone()
          .or_else(|| {
            base.language.clone()
          }),
      content_type:
        override_with
          .content_type
          .clone()
          .or_else(|| {
            base.content_type.clone()
          }),
      cookie_path:
        override_with
          .cookie_path
          .clone()
          .or_else(|| {
            base.cookie_path.clone()
          }),
      check_method:
        override_with
          .check_method
          .clone()
          .or_else(|| {
            base.check_method.clone()
          }),
      fallback_to_get:
        override_with
          .fallback_to_get
          .or(base.fallback_to_get),
      detectors:
        override_with
          .detectors
          .clone()
          .or_else(|| {
            base.detectors.clone()
          }),
      fetch_body_on_change:
        override_with
          .fetch_body_on_change
          .or(base.fetch_body_on_change),
      max_body_bytes:
        override_with
          .max_body_bytes
          .or(base.max_body_bytes),
      item_selector:
        override_with
          .item_selector
          .clone()
          .or_else(|| {
            base.item_selector.clone()
          }),
      item_identity:
        override_with
          .item_identity
          .clone()
          .or_else(|| {
            base.item_identity.clone()
          }),
      item_identity_attr:
        override_with
          .item_identity_attr
          .clone()
          .or_else(|| {
            base
              .item_identity_attr
              .clone()
          }),
      title_selector:
        override_with
          .title_selector
          .clone()
          .or_else(|| {
            base.title_selector.clone()
          }),
      link_selector:
        override_with
          .link_selector
          .clone()
          .or_else(|| {
            base.link_selector.clone()
          }),
      summary_selector:
        override_with
          .summary_selector
          .clone()
          .or_else(|| {
            base
              .summary_selector
              .clone()
          }),
      published_selector:
        override_with
          .published_selector
          .clone()
          .or_else(|| {
            base
              .published_selector
              .clone()
          }),
      published_format:
        override_with
          .published_format
          .clone()
          .or_else(|| {
            base
              .published_format
              .clone()
          }),
      include_selectors:
        override_with
          .include_selectors
          .clone()
          .or_else(|| {
            base
              .include_selectors
              .clone()
          }),
      exclude_selectors:
        override_with
          .exclude_selectors
          .clone()
          .or_else(|| {
            base
              .exclude_selectors
              .clone()
          }),
      normalize_whitespace:
        override_with
          .normalize_whitespace
          .or(base.normalize_whitespace),
      strip_query_params:
        override_with
          .strip_query_params
          .or(base.strip_query_params),
      emit_mode:
        override_with
          .emit_mode
          .clone()
          .or_else(|| {
            base.emit_mode.clone()
          }),
      emit_title:
        override_with
          .emit_title
          .clone()
          .or_else(|| {
            base.emit_title.clone()
          }),
      min_item_count_change:
        override_with
          .min_item_count_change
          .or(
            base.min_item_count_change
          )
    }
  }
}

fn apply_feed_defaults(
  mut feed: super::raw::RawFeed,
  defaults: &FeedDefaults,
  path: &Path
) -> Result<
  super::raw::RawFeed,
  ConfigError
> {
  if feed.base_poll_seconds.is_none() {
    feed.base_poll_seconds =
      defaults.base_poll_seconds;
  }

  if feed.category.is_none() {
    feed.category =
      defaults.category.clone();
  }

  if feed.provenance.is_none() {
    feed.provenance =
      defaults.provenance.clone();
  }

  if feed.tags.is_none() {
    feed.tags = defaults.tags.clone();
  }

  if feed.language.is_none() {
    feed.language =
      defaults.language.clone();
  }

  if feed.content_type.is_none() {
    feed.content_type =
      defaults.content_type.clone();
  }

  if feed.cookie_path.is_none() {
    feed.cookie_path =
      defaults.cookie_path.clone();
  }

  let prefix = match feed
    .id_prefix
    .as_deref()
  {
    | Some(raw) => {
      let normalized =
        validate_id_prefix(
          raw,
          &format!(
            "feed '{}' in {}",
            feed.id,
            path.display()
          )
        )?;

      feed.id_prefix =
        Some(normalized.clone());

      Some(normalized)
    }
    | None => defaults.id_prefix.clone()
  };

  if let Some(prefix) = prefix {
    feed.id =
      format!("{prefix}-{}", feed.id);
  }

  Ok(feed)
}

fn apply_watch_defaults(
  mut watch: RawWatch,
  defaults: &WatchDefaults,
  profiles: &HashMap<
    String,
    WatchDefaults
  >,
  path: &Path
) -> Result<RawWatch, ConfigError> {
  let base = if let Some(profile_name) =
    watch.profile.as_deref()
  {
    let key = profile_name.trim();

    profiles
      .get(key)
      .cloned()
      .ok_or_else(|| {
        ConfigError::Invalid(format!(
          "watch '{}' in {} \
           references unknown profile \
           '{}'",
          watch.id,
          path.display(),
          key
        ))
      })?
  } else {
    defaults.clone()
  };

  if watch.base_poll_seconds.is_none() {
    watch.base_poll_seconds =
      base.base_poll_seconds;
  }

  if watch.category.is_none() {
    watch.category =
      base.category.clone();
  }

  if watch.provenance.is_none() {
    watch.provenance =
      base.provenance.clone();
  }

  if watch.tags.is_none() {
    watch.tags = base.tags.clone();
  }

  if watch.language.is_none() {
    watch.language =
      base.language.clone();
  }

  if watch.content_type.is_none() {
    watch.content_type =
      base.content_type.clone();
  }

  if watch.cookie_path.is_none() {
    watch.cookie_path =
      base.cookie_path.clone();
  }

  if watch.check_method.is_none() {
    watch.check_method =
      base.check_method.clone();
  }

  if watch.fallback_to_get.is_none() {
    watch.fallback_to_get =
      base.fallback_to_get;
  }

  if watch.detectors.is_none() {
    watch.detectors =
      base.detectors.clone();
  }

  if watch
    .fetch_body_on_change
    .is_none()
  {
    watch.fetch_body_on_change =
      base.fetch_body_on_change;
  }

  if watch.max_body_bytes.is_none() {
    watch.max_body_bytes =
      base.max_body_bytes;
  }

  if watch.item_selector.is_none() {
    watch.item_selector =
      base.item_selector.clone();
  }

  if watch.item_identity.is_none() {
    watch.item_identity =
      base.item_identity.clone();
  }

  if watch.item_identity_attr.is_none()
  {
    watch.item_identity_attr =
      base.item_identity_attr.clone();
  }

  if watch.title_selector.is_none() {
    watch.title_selector =
      base.title_selector.clone();
  }

  if watch.link_selector.is_none() {
    watch.link_selector =
      base.link_selector.clone();
  }

  if watch.summary_selector.is_none() {
    watch.summary_selector =
      base.summary_selector.clone();
  }

  if watch.published_selector.is_none()
  {
    watch.published_selector =
      base.published_selector.clone();
  }

  if watch.published_format.is_none() {
    watch.published_format =
      base.published_format.clone();
  }

  if watch.include_selectors.is_none() {
    watch.include_selectors =
      base.include_selectors.clone();
  }

  if watch.exclude_selectors.is_none() {
    watch.exclude_selectors =
      base.exclude_selectors.clone();
  }

  if watch
    .normalize_whitespace
    .is_none()
  {
    watch.normalize_whitespace =
      base.normalize_whitespace;
  }

  if watch.strip_query_params.is_none()
  {
    watch.strip_query_params =
      base.strip_query_params;
  }

  if watch.emit_mode.is_none() {
    watch.emit_mode =
      base.emit_mode.clone();
  }

  if watch.emit_title.is_none() {
    watch.emit_title =
      base.emit_title.clone();
  }

  if watch
    .min_item_count_change
    .is_none()
  {
    watch.min_item_count_change =
      base.min_item_count_change;
  }

  let prefix =
    match watch.id_prefix.as_deref() {
      | Some(raw) => {
        let normalized =
          validate_id_prefix(
            raw,
            &format!(
              "watch '{}' in {}",
              watch.id,
              path.display()
            )
          )?;

        watch.id_prefix =
          Some(normalized.clone());

        Some(normalized)
      }
      | None => base.id_prefix.clone()
    };

  if let Some(prefix) = prefix {
    watch.id =
      format!("{prefix}-{}", watch.id);
  }

  Ok(watch)
}

fn validate_id_prefix(
  prefix: &str,
  context: &str
) -> Result<String, ConfigError> {
  let trimmed = prefix.trim();

  if trimmed.is_empty() {
    return Err(ConfigError::Invalid(
      format!(
        "{context} id_prefix cannot \
         be empty"
      )
    ));
  }

  if !trimmed.chars().all(|c| {
    c.is_ascii_lowercase()
      || c.is_ascii_digit()
  }) {
    return Err(ConfigError::Invalid(
      format!(
        "{context} id_prefix must be \
         lowercase alphanumeric"
      )
    ));
  }

  Ok(trimmed.to_string())
}
