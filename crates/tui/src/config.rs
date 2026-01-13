use std::path::{
  Path,
  PathBuf
};

use crossterm::event::{
  KeyCode,
  KeyModifiers
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct TuiConfig {
  pub(crate) server:      ServerConfig,
  pub(crate) auth:        AuthConfig,
  pub(crate) ui:          UiConfig,
  pub(crate) keybindings: Keybindings
}

#[derive(Debug, Deserialize)]
pub(crate) struct ServerConfig {
  pub(crate) url:        String,
  pub(crate) timeout_ms: u64
}

#[derive(Debug, Deserialize)]
pub(crate) struct AuthConfig {
  pub(crate) auto_login: bool,
  pub(crate) username:   String,
  pub(crate) password:   String
}

#[derive(Debug, Deserialize)]
pub(crate) struct UiConfig {
  pub(crate) hide_empty_feeds: bool,
  pub(crate) hide_read_feeds: bool,
  pub(crate) entries_page_size: u32,
  pub(crate) feeds_page_size: u32,
  pub(crate) favorites_page_size: u32,
  pub(crate) folders_page_size: u32,
  pub(crate) subscriptions_page_size:
    u32,
  pub(crate) refresh_interval_ms: u64
}

#[derive(Debug, Deserialize)]
pub(crate) struct Keybindings {
  pub(crate) quit: String,
  pub(crate) refresh: String,
  pub(crate) next_tab: String,
  pub(crate) prev_tab: String,
  pub(crate) tab_feeds: String,
  pub(crate) tab_entries: String,
  pub(crate) tab_favorites: String,
  pub(crate) tab_folders: String,
  pub(crate) tab_subscriptions: String,
  pub(crate) move_down: String,
  pub(crate) move_up: String,
  pub(crate) go_top: String,
  pub(crate) go_middle: String,
  pub(crate) go_bottom: String,
  pub(crate) open_category_menu: String,
  pub(crate) open_tag_menu: String,
  pub(crate) open_sort_menu: String,
  pub(crate) clear_filters: String,
  pub(crate) open_entries: String,
  pub(crate) open_all_entries: String,
  pub(crate) open_search: String,
  pub(crate) toggle_entries_filter:
    String,
  pub(crate) toggle_read: String,
  pub(crate) toggle_subscribe: String,
  pub(crate) toggle_favorite: String,
  pub(crate) toggle_hide_empty: String,
  pub(crate) toggle_hide_read: String,
  pub(crate) entries_next: String,
  pub(crate) entries_prev: String,
  pub(crate) feeds_next: String,
  pub(crate) feeds_prev: String,
  pub(crate) folder_create: String,
  pub(crate) folder_rename: String,
  pub(crate) folder_delete: String,
  pub(crate) folder_assign: String,
  pub(crate) folder_unassign: String
}

#[derive(Debug, Clone)]
pub(crate) struct KeyBinding {
  pub(crate) code:      KeyCode,
  pub(crate) modifiers: KeyModifiers
}

pub(crate) struct ResolvedKeybindings {
  pub(crate) quit: KeyBinding,
  pub(crate) refresh: KeyBinding,
  pub(crate) next_tab: KeyBinding,
  pub(crate) prev_tab: KeyBinding,
  pub(crate) tab_feeds: KeyBinding,
  pub(crate) tab_entries: KeyBinding,
  pub(crate) tab_favorites: KeyBinding,
  pub(crate) tab_folders: KeyBinding,
  pub(crate) tab_subscriptions:
    KeyBinding,
  pub(crate) move_down: KeyBinding,
  pub(crate) move_up: KeyBinding,
  pub(crate) go_top: KeyBinding,
  pub(crate) go_middle: KeyBinding,
  pub(crate) go_bottom: KeyBinding,
  pub(crate) open_category_menu:
    KeyBinding,
  pub(crate) open_tag_menu: KeyBinding,
  pub(crate) open_sort_menu: KeyBinding,
  pub(crate) clear_filters: KeyBinding,
  pub(crate) open_entries: KeyBinding,
  pub(crate) open_all_entries:
    KeyBinding,
  pub(crate) open_search: KeyBinding,
  pub(crate) toggle_entries_filter:
    KeyBinding,
  pub(crate) toggle_read: KeyBinding,
  pub(crate) toggle_subscribe:
    KeyBinding,
  pub(crate) toggle_favorite:
    KeyBinding,
  pub(crate) toggle_hide_empty:
    KeyBinding,
  pub(crate) toggle_hide_read:
    KeyBinding,
  pub(crate) entries_next: KeyBinding,
  pub(crate) entries_prev: KeyBinding,
  pub(crate) feeds_next: KeyBinding,
  pub(crate) feeds_prev: KeyBinding,
  pub(crate) folder_create: KeyBinding,
  pub(crate) folder_rename: KeyBinding,
  pub(crate) folder_delete: KeyBinding,
  pub(crate) folder_assign: KeyBinding,
  pub(crate) folder_unassign:
    KeyBinding
}

#[derive(Debug)]
pub(crate) struct ConfigError(
  pub(crate) String
);

impl std::fmt::Display for ConfigError {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::error::Error for ConfigError {}

impl TuiConfig {
  pub(crate) fn load(
    path: &Path
  ) -> Result<Self, ConfigError> {
    let base_dir = path
      .parent()
      .ok_or_else(|| {
        ConfigError(
          "config path has no parent"
            .into()
        )
      })?;

    let schema_path = base_dir
      .join("schemas")
      .join("tui.schema.json");

    let schema =
      std::fs::read_to_string(
        &schema_path
      )
      .map_err(|_| {
        ConfigError(format!(
          "schema not found at {}",
          schema_path.display()
        ))
      })?;

    let content =
      std::fs::read_to_string(path)
        .map_err(|e| {
          ConfigError(format!(
            "config IO error: {e}"
          ))
        })?;

    validate_toml(
      &schema,
      &content,
      &path.display().to_string()
    )?;

    let config: TuiConfig =
      toml::from_str(&content)
        .map_err(|e| {
          ConfigError(format!(
            "config parse error: {e}"
          ))
        })?;

    Ok(config)
  }

  pub(crate) fn resolved_keybindings(
    &self
  ) -> Result<
    ResolvedKeybindings,
    ConfigError
  > {
    Ok(ResolvedKeybindings {
      quit:                  parse_key(
        &self.keybindings.quit
      )?,
      refresh:               parse_key(
        &self.keybindings.refresh
      )?,
      next_tab:              parse_key(
        &self.keybindings.next_tab
      )?,
      prev_tab:              parse_key(
        &self.keybindings.prev_tab
      )?,
      tab_feeds:             parse_key(
        &self.keybindings.tab_feeds
      )?,
      tab_entries:           parse_key(
        &self.keybindings.tab_entries
      )?,
      tab_favorites:         parse_key(
        &self.keybindings.tab_favorites
      )?,
      tab_folders:           parse_key(
        &self.keybindings.tab_folders
      )?,
      tab_subscriptions:     parse_key(
        &self
          .keybindings
          .tab_subscriptions
      )?,
      move_down:             parse_key(
        &self.keybindings.move_down
      )?,
      move_up:               parse_key(
        &self.keybindings.move_up
      )?,
      go_top:                parse_key(
        &self.keybindings.go_top
      )?,
      go_middle:             parse_key(
        &self.keybindings.go_middle
      )?,
      go_bottom:             parse_key(
        &self.keybindings.go_bottom
      )?,
      open_category_menu:    parse_key(
        &self
          .keybindings
          .open_category_menu
      )?,
      open_tag_menu:         parse_key(
        &self.keybindings.open_tag_menu
      )?,
      open_sort_menu:        parse_key(
        &self
          .keybindings
          .open_sort_menu
      )?,
      clear_filters:         parse_key(
        &self.keybindings.clear_filters
      )?,
      open_entries:          parse_key(
        &self.keybindings.open_entries
      )?,
      open_all_entries:      parse_key(
        &self
          .keybindings
          .open_all_entries
      )?,
      open_search:           parse_key(
        &self.keybindings.open_search
      )?,
      toggle_entries_filter: parse_key(
        &self
          .keybindings
          .toggle_entries_filter
      )?,
      toggle_read:           parse_key(
        &self.keybindings.toggle_read
      )?,
      toggle_subscribe:      parse_key(
        &self
          .keybindings
          .toggle_subscribe
      )?,
      toggle_favorite:       parse_key(
        &self
          .keybindings
          .toggle_favorite
      )?,
      toggle_hide_empty:     parse_key(
        &self
          .keybindings
          .toggle_hide_empty
      )?,
      toggle_hide_read:      parse_key(
        &self
          .keybindings
          .toggle_hide_read
      )?,
      entries_next:          parse_key(
        &self.keybindings.entries_next
      )?,
      entries_prev:          parse_key(
        &self.keybindings.entries_prev
      )?,
      feeds_next:            parse_key(
        &self.keybindings.feeds_next
      )?,
      feeds_prev:            parse_key(
        &self.keybindings.feeds_prev
      )?,
      folder_create:         parse_key(
        &self.keybindings.folder_create
      )?,
      folder_rename:         parse_key(
        &self.keybindings.folder_rename
      )?,
      folder_delete:         parse_key(
        &self.keybindings.folder_delete
      )?,
      folder_assign:         parse_key(
        &self.keybindings.folder_assign
      )?,
      folder_unassign:       parse_key(
        &self
          .keybindings
          .folder_unassign
      )?
    })
  }
}

pub(crate) fn default_config_path()
-> PathBuf {
  PathBuf::from(
    "crates/tui/res/config.toml"
  )
}

fn validate_toml(
  schema: &str,
  toml_input: &str,
  name: &str
) -> Result<(), ConfigError> {
  let schema_json: serde_json::Value =
    serde_json::from_str(schema)
      .map_err(|e| {
        ConfigError(format!(
          "schema parse error: {e}"
        ))
      })?;

  let compiled =
    jsonschema::validator_for(
      &schema_json
    )
    .map_err(|e| {
      ConfigError(format!(
        "schema compile error: {e}"
      ))
    })?;

  let toml_value: toml::Value =
    toml::from_str(toml_input)
      .map_err(|e| {
        ConfigError(format!(
          "{name}: {e}"
        ))
      })?;

  let json_value =
    serde_json::to_value(toml_value)
      .map_err(|e| {
        ConfigError(e.to_string())
      })?;

  let mut errors =
    compiled.iter_errors(&json_value);

  if let Some(err) = errors.next() {
    let mut messages =
      vec![err.to_string()];
    for e in errors.take(4) {
      messages.push(e.to_string());
    }

    return Err(ConfigError(format!(
      "schema validation failed for \
       {name}: {}",
      messages.join("; ")
    )));
  }

  Ok(())
}

fn parse_key(
  raw: &str
) -> Result<KeyBinding, ConfigError> {
  let raw = raw.trim();
  if raw.is_empty() {
    return Err(ConfigError(
      "empty keybinding".into()
    ));
  }

  let mut modifiers =
    KeyModifiers::NONE;
  let mut key = raw.to_string();

  if let Some(rest) =
    key.strip_prefix("ctrl+")
  {
    modifiers |= KeyModifiers::CONTROL;
    key = rest.to_string();
  }

  let code = match key.as_str() {
    | "left" => KeyCode::Left,
    | "right" => KeyCode::Right,
    | "up" => KeyCode::Up,
    | "down" => KeyCode::Down,
    | "tab" => KeyCode::Tab,
    | "enter" => KeyCode::Enter,
    | "backspace" => KeyCode::Backspace,
    | "esc" => KeyCode::Esc,
    | _ => {
      if key.chars().count() == 1 {
        let ch =
          key.chars().next().unwrap();
        if ch.is_ascii_uppercase()
          && !modifiers.contains(
            KeyModifiers::SHIFT
          )
        {
          modifiers |=
            KeyModifiers::SHIFT;
        }
        KeyCode::Char(ch)
      } else {
        return Err(ConfigError(
          format!(
            "unsupported keybinding              '{raw}'"
          )
        ));
      }
    }
  };

  Ok(KeyBinding {
    code,
    modifiers
  })
}
