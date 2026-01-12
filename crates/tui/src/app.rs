use anyhow::{
  Context,
  Result
};
use crossterm::event::{
  KeyCode,
  KeyEvent,
  KeyModifiers
};
use reqwest::blocking::Client;

use crate::config::{
  KeyBinding,
  ResolvedKeybindings,
  TuiConfig
};
use crate::models::{
  EntryListResponse,
  EntrySummary,
  FeedSummary,
  FolderRow,
  TokenResponse
};

#[derive(
  Debug, Clone, Copy, PartialEq, Eq,
)]
pub(crate) enum Screen {
  Login,
  Main
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum LoginField {
  Username,
  Password
}

pub(crate) struct App {
  pub(crate) screen: Screen,
  pub(crate) focus: LoginField,
  pub(crate) username: String,
  pub(crate) password: String,
  pub(crate) status: String,
  pub(crate) token: Option<String>,
  pub(crate) feeds: Vec<FeedSummary>,
  pub(crate) favorites:
    Vec<FeedSummary>,
  pub(crate) folders: Vec<FolderRow>,
  pub(crate) entries: Vec<EntrySummary>,
  pub(crate) tab: usize,
  pub(crate) selected_feed: usize,
  pub(crate) selected_entry: usize,
  pub(crate) selected_favorite: usize,
  pub(crate) selected_folder: usize,
  pub(crate) page_size: u32,
  pub(crate) keys: ResolvedKeybindings,
  entries_feed_id: Option<String>,
  entries_offset: i64,
  entries_next_offset: Option<i64>,
  base_url: String,
  client: Client
}

impl App {
  pub(crate) fn new(
    config: &TuiConfig,
    keys: ResolvedKeybindings
  ) -> Result<Self> {
    let client = Client::builder()
      .timeout(std::time::Duration::from_millis(
        config.server.timeout_ms,
      ))
      .build()?;

    Ok(Self {
      screen: Screen::Login,
      focus: LoginField::Username,
      username: config
        .auth
        .username
        .clone(),
      password: config
        .auth
        .password
        .clone(),
      status: "Enter credentials. Tab \
               switches fields. Enter \
               to login."
        .to_string(),
      token: None,
      feeds: Vec::new(),
      favorites: Vec::new(),
      folders: Vec::new(),
      entries: Vec::new(),
      tab: 0,
      selected_feed: 0,
      selected_entry: 0,
      selected_favorite: 0,
      selected_folder: 0,
      page_size: config.ui.page_size,
      keys,
      entries_feed_id: None,
      entries_offset: 0,
      entries_next_offset: None,
      base_url: config
        .server
        .url
        .clone(),
      client
    })
  }

  pub(crate) fn handle_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    match self.screen {
      | Screen::Login => {
        self.handle_login_key(key)
      }
      | Screen::Main => {
        self.handle_main_key(key)
      }
    }
  }

  fn handle_login_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    if self
      .key_matches(&self.keys.quit, key)
      || (key.code
        == KeyCode::Char('c')
        && key.modifiers
          == KeyModifiers::CONTROL)
    {
      return Ok(true);
    }

    match key {
      | KeyEvent {
        code: KeyCode::Tab,
        ..
      } => {
        self.focus = match self.focus {
          | LoginField::Username => {
            LoginField::Password
          }
          | LoginField::Password => {
            LoginField::Username
          }
        };
      }
      | KeyEvent {
        code: KeyCode::Enter,
        ..
      } => {
        self.login()?;
      }
      | KeyEvent {
        code: KeyCode::Backspace,
        ..
      } => {
        match self.focus {
          | LoginField::Username => {
            self.username.pop();
          }
          | LoginField::Password => {
            self.password.pop();
          }
        }
      }
      | KeyEvent {
        code: KeyCode::Char(ch),
        modifiers: KeyModifiers::NONE,
        ..
      } => {
        match self.focus {
          | LoginField::Username => {
            self.username.push(ch)
          }
          | LoginField::Password => {
            self.password.push(ch)
          }
        }
      }
      | _ => {}
    }

    Ok(false)
  }

  fn handle_main_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    if self
      .key_matches(&self.keys.quit, key)
      || (key.code
        == KeyCode::Char('c')
        && key.modifiers
          == KeyModifiers::CONTROL)
    {
      return Ok(true);
    }

    if self.key_matches(
      &self.keys.tab_feeds,
      key
    ) {
      self.tab = 0;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.tab_entries,
      key
    ) {
      self.tab = 1;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.tab_favorites,
      key
    ) {
      self.tab = 2;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.tab_folders,
      key
    ) {
      self.tab = 3;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.prev_tab,
      key
    ) {
      self.tab = if self.tab == 0 {
        3
      } else {
        self.tab - 1
      };
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.next_tab,
      key
    ) {
      self.tab = (self.tab + 1) % 4;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.refresh,
      key
    ) {
      self.refresh_tab()?;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.move_down,
      key
    ) {
      self.move_selection(1);
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.move_up,
      key
    ) {
      self.move_selection(-1);
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.open_entries,
      key
    ) {
      self.open_entries()?;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.toggle_read,
      key
    ) {
      self.toggle_entry_read()?;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.entries_next,
      key
    ) {
      self.next_entries_page()?;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.entries_prev,
      key
    ) {
      self.prev_entries_page()?;
      return Ok(false);
    }

    Ok(false)
  }

  pub(crate) fn login(
    &mut self
  ) -> Result<()> {
    let url = format!(
      "{}/v1/auth/login",
      self.base_url
    );

    let body = serde_json::json!({
      "username": &self.username,
      "password": &self.password,
    });

    let resp = self
      .client
      .post(url)
      .json(&body)
      .send()
      .context(
        "login request failed"
      )?;

    if !resp.status().is_success() {
      let msg = resp
        .text()
        .unwrap_or_else(|_| {
          "login failed".to_string()
        });

      self.status =
        format!("Login failed: {msg}");

      return Ok(());
    }

    let token = resp
      .json::<TokenResponse>()?
      .token;

    self.token = Some(token);
    self.screen = Screen::Main;
    self.status = "Logged in. Press r \
                   to refresh."
      .to_string();

    self.refresh_all()?;

    Ok(())
  }

  pub(crate) fn refresh_all(
    &mut self
  ) -> Result<()> {
    self.refresh_feeds()?;
    self.refresh_favorites()?;
    self.refresh_folders()?;

    if self.entries_feed_id.is_some() {
      self.refresh_entries()?;
    }

    Ok(())
  }

  pub(crate) fn refresh_tab(
    &mut self
  ) -> Result<()> {
    match self.tab {
      | 0 => self.refresh_feeds(),
      | 1 => self.refresh_entries(),
      | 2 => self.refresh_favorites(),
      | _ => self.refresh_folders()
    }
  }

  fn refresh_feeds(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/feeds",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "feeds request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load feeds ({})",
        resp.status()
      );

      return Ok(());
    }

    self.feeds = resp.json().context(
      "failed to parse feeds"
    )?;

    if self.selected_feed
      >= self.feeds.len()
    {
      self.selected_feed = 0;
    }

    self.status = format!(
      "Loaded {} feeds",
      self.feeds.len()
    );

    Ok(())
  }

  fn refresh_favorites(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/favorites",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "favorites request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load favorites ({})",
        resp.status()
      );

      return Ok(());
    }

    self.favorites =
      resp.json().context(
        "failed to parse favorites"
      )?;

    if self.selected_favorite
      >= self.favorites.len()
    {
      self.selected_favorite = 0;
    }

    self.status = format!(
      "Loaded {} favorites",
      self.favorites.len()
    );

    Ok(())
  }

  fn refresh_folders(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/folders",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "folders request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load folders ({})",
        resp.status()
      );

      return Ok(());
    }

    self.folders =
      resp.json().context(
        "failed to parse folders"
      )?;

    if self.selected_folder
      >= self.folders.len()
    {
      self.selected_folder = 0;
    }

    self.status = format!(
      "Loaded {} folders",
      self.folders.len()
    );

    Ok(())
  }

  fn refresh_entries(
    &mut self
  ) -> Result<()> {
    let Some(feed_id) =
      self.entries_feed_id.clone()
    else {
      self.status = "Select a feed \
                     and press the \
                     entries key."
        .to_string();
      return Ok(());
    };

    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/feeds/{}/entries?\
       limit={}&offset={}&read=all",
      self.base_url,
      feed_id,
      self.page_size,
      self.entries_offset
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "entries request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load entries ({})",
        resp.status()
      );
      return Ok(());
    }

    let data = resp
      .json::<EntryListResponse>()
      .context(
        "failed to parse entries"
      )?;

    self.entries = data.items;
    self.entries_next_offset =
      data.next_offset;

    if self.selected_entry
      >= self.entries.len()
    {
      self.selected_entry = 0;
    }

    self.status = format!(
      "Loaded {} entries",
      self.entries.len()
    );

    Ok(())
  }

  fn open_entries(
    &mut self
  ) -> Result<()> {
    if self.feeds.is_empty() {
      self.status =
        "No feeds loaded".to_string();
      return Ok(());
    }

    let feed = self
      .feeds
      .get(self.selected_feed)
      .cloned();

    if let Some(feed) = feed {
      self.entries_feed_id =
        Some(feed.id);
      self.entries_offset = 0;
      self.selected_entry = 0;
      self.tab = 1;
      self.refresh_entries()?;
    }

    Ok(())
  }

  fn next_entries_page(
    &mut self
  ) -> Result<()> {
    if self.tab != 1 {
      return Ok(());
    }

    if let Some(next) =
      self.entries_next_offset
    {
      self.entries_offset = next;
      self.refresh_entries()?;
    }

    Ok(())
  }

  fn prev_entries_page(
    &mut self
  ) -> Result<()> {
    if self.tab != 1 {
      return Ok(());
    }

    if self.entries_offset == 0 {
      return Ok(());
    }

    let size = self.page_size as i64;
    self.entries_offset =
      (self.entries_offset - size)
        .max(0);
    self.refresh_entries()?;

    Ok(())
  }

  fn toggle_entry_read(
    &mut self
  ) -> Result<()> {
    if self.tab != 1 {
      return Ok(());
    }

    let entry = match self
      .entries
      .get(self.selected_entry)
    {
      | Some(entry) => entry.clone(),
      | None => return Ok(())
    };

    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/entries/{}/read",
      self.base_url, entry.id
    );

    let req = if entry.is_read {
      self.client.delete(url)
    } else {
      self.client.post(url)
    };

    let resp = req
      .bearer_auth(token)
      .send()
      .context("toggle read failed")?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to update read state \
         ({})",
        resp.status()
      );
      return Ok(());
    }

    if let Some(row) = self
      .entries
      .get_mut(self.selected_entry)
    {
      row.is_read = !entry.is_read;
    }

    Ok(())
  }

  pub(crate) fn key_matches(
    &self,
    binding: &KeyBinding,
    key: KeyEvent
  ) -> bool {
    key.code == binding.code
      && key.modifiers
        == binding.modifiers
  }

  fn move_selection(
    &mut self,
    delta: i32
  ) {
    match self.tab {
      | 0 => {
        let len = self.feeds.len();
        self.selected_feed = move_index(
          self.selected_feed,
          len,
          delta
        );
      }
      | 1 => {
        let len = self.entries.len();
        self.selected_entry =
          move_index(
            self.selected_entry,
            len,
            delta
          );
      }
      | 2 => {
        let len = self.favorites.len();
        self.selected_favorite =
          move_index(
            self.selected_favorite,
            len,
            delta
          );
      }
      | _ => {
        let len = self.folders.len();
        self.selected_folder =
          move_index(
            self.selected_folder,
            len,
            delta
          );
      }
    }
  }
}

fn move_index(
  current: usize,
  len: usize,
  delta: i32
) -> usize {
  if len == 0 {
    return 0;
  }

  let max =
    len.saturating_sub(1) as i32;
  let next = (current as i32 + delta)
    .clamp(0, max);

  next as usize
}
