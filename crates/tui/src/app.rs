use std::collections::{
  BTreeSet,
  HashMap,
  HashSet
};

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
  FeedEntryCounts,
  FeedSummary,
  FolderRow,
  SubscriptionRow,
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
  pub(crate) subscriptions:
    HashSet<String>,
  pub(crate) feed_counts:
    HashMap<String, FeedEntryCounts>,
  pub(crate) feeds_view: Vec<usize>,
  pub(crate) subscriptions_view:
    Vec<usize>,
  pub(crate) categories: Vec<String>,
  pub(crate) tags: Vec<String>,
  pub(crate) filter_category:
    Option<usize>,
  pub(crate) filter_tag: Option<usize>,
  pub(crate) entries: Vec<EntrySummary>,
  pub(crate) tab: usize,
  pub(crate) selected_feed: usize,
  pub(crate) selected_entry: usize,
  pub(crate) selected_favorite: usize,
  pub(crate) selected_folder: usize,
  pub(crate) selected_subscription:
    usize,
  pub(crate) page_size: u32,
  pub(crate) feeds_offset: usize,
  pub(crate) favorites_offset: usize,
  pub(crate) folders_offset: usize,
  pub(crate) subscriptions_offset:
    usize,
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
      subscriptions: HashSet::new(),
      feed_counts: HashMap::new(),
      feeds_view: Vec::new(),
      subscriptions_view: Vec::new(),
      categories: Vec::new(),
      tags: Vec::new(),
      filter_category: None,
      filter_tag: None,
      entries: Vec::new(),
      tab: 0,
      selected_feed: 0,
      selected_entry: 0,
      selected_favorite: 0,
      selected_folder: 0,
      selected_subscription: 0,
      page_size: config.ui.page_size,
      feeds_offset: 0,
      favorites_offset: 0,
      folders_offset: 0,
      subscriptions_offset: 0,
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
      &self.keys.tab_subscriptions,
      key
    ) {
      self.tab = 4;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.prev_tab,
      key
    ) {
      self.tab = if self.tab == 0 {
        4
      } else {
        self.tab - 1
      };
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.next_tab,
      key
    ) {
      self.tab = (self.tab + 1) % 5;
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
      &self.keys.filter_category_next,
      key
    ) {
      self.advance_category_filter(1);
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.filter_category_prev,
      key
    ) {
      self.advance_category_filter(-1);
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.filter_tag_next,
      key
    ) {
      self.advance_tag_filter(1);
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.filter_tag_prev,
      key
    ) {
      self.advance_tag_filter(-1);
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.clear_filters,
      key
    ) {
      self.clear_filters();
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
      &self.keys.toggle_subscribe,
      key
    ) {
      self.toggle_subscribe()?;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.entries_next,
      key
    ) {
      if self.tab == 1 {
        self.next_entries_page()?;
      } else {
        self.next_list_page();
      }
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.entries_prev,
      key
    ) {
      if self.tab == 1 {
        self.prev_entries_page()?;
      } else {
        self.prev_list_page();
      }
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
    self.refresh_subscriptions()?;
    self.refresh_feed_counts()?;
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
      | 0 => {
        self.refresh_feeds()?;
        self.refresh_subscriptions()?;
        self.refresh_feed_counts()
      }
      | 1 => self.refresh_entries(),
      | 2 => self.refresh_favorites(),
      | 4 => {
        self.refresh_subscriptions()?;
        self.refresh_feed_counts()
      }
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

    self.rebuild_views();

    if self.selected_feed
      >= self.feeds_view.len()
    {
      self.selected_feed = 0;
      self.feeds_offset = 0;
    }

    self.status = format!(
      "Loaded {} feeds ({} shown)",
      self.feeds.len(),
      self.feeds_view.len()
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
      self.favorites_offset = 0;
    }

    self.favorites_offset =
      ensure_offset(
        self.selected_favorite,
        self.favorites_offset,
        self.page_size as usize,
        self.favorites.len()
      );

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
      self.folders_offset = 0;
    }

    self.folders_offset = ensure_offset(
      self.selected_folder,
      self.folders_offset,
      self.page_size as usize,
      self.folders.len()
    );

    self.status = format!(
      "Loaded {} folders",
      self.folders.len()
    );

    Ok(())
  }

  fn refresh_subscriptions(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/subscriptions",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "subscriptions request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load subscriptions \
         ({})",
        resp.status()
      );

      return Ok(());
    }

    let rows = resp
      .json::<Vec<SubscriptionRow>>()
      .context(
        "failed to parse subscriptions"
      )?;

    self.subscriptions = rows
      .into_iter()
      .map(|row| row.feed_id)
      .collect();

    Ok(())
  }

  fn refresh_feed_counts(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/feeds/counts",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "feed counts request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load feed counts \
         ({})",
        resp.status()
      );

      return Ok(());
    }

    let rows = resp
      .json::<Vec<FeedEntryCounts>>()
      .context(
        "failed to parse feed counts"
      )?;

    self.feed_counts = rows
      .into_iter()
      .map(|row| {
        (row.feed_id.clone(), row)
      })
      .collect();

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
      "Loaded {} entries (offset {})",
      self.entries.len(),
      self.entries_offset
    );

    Ok(())
  }

  fn open_entries(
    &mut self
  ) -> Result<()> {
    if self.feeds_view.is_empty() {
      self.status =
        "No feeds loaded".to_string();
      return Ok(());
    }

    let feed = self
      .feeds_view
      .get(self.selected_feed)
      .and_then(|idx| {
        self.feeds.get(*idx)
      })
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

  fn toggle_subscribe(
    &mut self
  ) -> Result<()> {
    if self.tab != 0 {
      return Ok(());
    }

    let feed = match self
      .feeds_view
      .get(self.selected_feed)
      .and_then(|idx| {
        self.feeds.get(*idx)
      }) {
      | Some(feed) => feed.clone(),
      | None => return Ok(())
    };

    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let subscribed = self
      .subscriptions
      .contains(&feed.id);

    let resp = if subscribed {
      let url = format!(
        "{}/v1/subscriptions/{}",
        self.base_url, feed.id
      );
      self
        .client
        .delete(url)
        .bearer_auth(token)
        .send()
        .context(
          "unsubscribe request failed"
        )?
    } else {
      let url = format!(
        "{}/v1/subscriptions",
        self.base_url
      );
      let body = serde_json::json!({
        "feed_id": feed.id,
      });
      self
        .client
        .post(url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .context(
          "subscribe request failed"
        )?
    };

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to update \
         subscription ({})",
        resp.status()
      );
      return Ok(());
    }

    if subscribed {
      self
        .subscriptions
        .remove(&feed.id);
      self.status = format!(
        "Unsubscribed from {}",
        feed.id
      );
    } else {
      self
        .subscriptions
        .insert(feed.id);
      self.status =
        "Subscribed".to_string();
    }

    Ok(())
  }

  fn rebuild_views(&mut self) {
    let mut categories =
      BTreeSet::new();
    let mut tags = BTreeSet::new();

    for feed in &self.feeds {
      categories
        .insert(feed.category.clone());
      if let Some(feed_tags) =
        &feed.tags
      {
        for tag in feed_tags {
          tags.insert(tag.clone());
        }
      }
    }

    self.categories =
      categories.into_iter().collect();
    self.tags =
      tags.into_iter().collect();

    if let Some(idx) =
      self.filter_category
    {
      if idx >= self.categories.len() {
        self.filter_category = None;
      }
    }

    if let Some(idx) = self.filter_tag {
      if idx >= self.tags.len() {
        self.filter_tag = None;
      }
    }

    self.feeds_view = self
      .feeds
      .iter()
      .enumerate()
      .filter(|(_, feed)| {
        self.matches_filters(feed)
      })
      .map(|(idx, _)| idx)
      .collect();

    self.subscriptions_view = self
      .feeds_view
      .iter()
      .copied()
      .filter(|idx| {
        self.subscriptions.contains(
          &self.feeds[*idx].id
        )
      })
      .collect();

    self.selected_feed =
      self.selected_feed.min(
        self
          .feeds_view
          .len()
          .saturating_sub(1)
      );
    self.selected_subscription =
      self.selected_subscription.min(
        self
          .subscriptions_view
          .len()
          .saturating_sub(1)
      );

    self.feeds_offset = ensure_offset(
      self.selected_feed,
      self.feeds_offset,
      self.page_size as usize,
      self.feeds_view.len()
    );
    self.subscriptions_offset =
      ensure_offset(
        self.selected_subscription,
        self.subscriptions_offset,
        self.page_size as usize,
        self.subscriptions_view.len()
      );
  }

  fn matches_filters(
    &self,
    feed: &FeedSummary
  ) -> bool {
    if let Some(idx) =
      self.filter_category
    {
      if self
        .categories
        .get(idx)
        .map(|c| c != &feed.category)
        .unwrap_or(true)
      {
        return false;
      }
    }

    if let Some(idx) = self.filter_tag {
      let Some(tag) =
        self.tags.get(idx)
      else {
        return false;
      };

      let matches = feed
        .tags
        .as_ref()
        .map(|tags| {
          tags.iter().any(|t| t == tag)
        })
        .unwrap_or(false);

      if !matches {
        return false;
      }
    }

    true
  }

  fn advance_category_filter(
    &mut self,
    delta: i32
  ) {
    self.filter_category =
      advance_filter_index(
        self.filter_category,
        self.categories.len(),
        delta
      );
    self.apply_filter_change();
  }

  fn advance_tag_filter(
    &mut self,
    delta: i32
  ) {
    self.filter_tag =
      advance_filter_index(
        self.filter_tag,
        self.tags.len(),
        delta
      );
    self.apply_filter_change();
  }

  fn clear_filters(&mut self) {
    self.filter_category = None;
    self.filter_tag = None;
    self.apply_filter_change();
  }

  fn apply_filter_change(&mut self) {
    self.selected_feed = 0;
    self.selected_subscription = 0;
    self.feeds_offset = 0;
    self.subscriptions_offset = 0;
    self.rebuild_views();
    self.status = format!(
      "Filters: {}",
      self.filter_summary()
    );
  }

  fn filter_summary(&self) -> String {
    let category = self
      .filter_category
      .and_then(|idx| {
        self.categories.get(idx)
      })
      .cloned()
      .unwrap_or_else(|| {
        "all".to_string()
      });

    let tag = self
      .filter_tag
      .and_then(|idx| {
        self.tags.get(idx)
      })
      .cloned()
      .unwrap_or_else(|| {
        "all".to_string()
      });

    format!(
      "category={category} tag={tag}"
    )
  }

  fn next_list_page(&mut self) {
    let len = self.list_len_for_tab();
    if len == 0 {
      return;
    }

    let page = self.page_size as usize;
    let offset =
      self.list_offset_value();
    if offset + page >= len {
      return;
    }

    let next = (offset + page)
      .min(len.saturating_sub(1));
    self.set_list_offset(next);
    self
      .update_selected_for_offset(next);
    self.status =
      format!("Page offset {}", next);
  }

  fn prev_list_page(&mut self) {
    let len = self.list_len_for_tab();
    if len == 0 {
      return;
    }

    let page = self.page_size as usize;
    let offset =
      self.list_offset_value();
    if offset == 0 {
      return;
    }

    let next =
      offset.saturating_sub(page);
    self.set_list_offset(next);
    self
      .update_selected_for_offset(next);
    self.status =
      format!("Page offset {}", next);
  }

  fn update_selected_for_offset(
    &mut self,
    offset: usize
  ) {
    let page = self.page_size as usize;
    let selected =
      self.selected_for_tab();
    if *selected < offset {
      *selected = offset;
    } else if *selected >= offset + page
    {
      *selected = offset;
    }
  }

  fn list_len_for_tab(&self) -> usize {
    match self.tab {
      | 0 => self.feeds_view.len(),
      | 2 => self.favorites.len(),
      | 3 => self.folders.len(),
      | 4 => {
        self.subscriptions_view.len()
      }
      | _ => 0
    }
  }

  fn list_offset_value(&self) -> usize {
    match self.tab {
      | 0 => self.feeds_offset,
      | 2 => self.favorites_offset,
      | 3 => self.folders_offset,
      | 4 => self.subscriptions_offset,
      | _ => 0
    }
  }

  fn set_list_offset(
    &mut self,
    value: usize
  ) {
    match self.tab {
      | 0 => self.feeds_offset = value,
      | 2 => {
        self.favorites_offset = value
      }
      | 3 => {
        self.folders_offset = value
      }
      | 4 => {
        self.subscriptions_offset =
          value
      }
      | _ => {}
    }
  }

  fn list_offset_for_tab(
    &mut self
  ) -> &mut usize {
    match self.tab {
      | 0 => &mut self.feeds_offset,
      | 2 => &mut self.favorites_offset,
      | 3 => &mut self.folders_offset,
      | 4 => {
        &mut self.subscriptions_offset
      }
      | _ => &mut self.feeds_offset
    }
  }

  fn selected_value_for_tab(
    &self
  ) -> usize {
    match self.tab {
      | 0 => self.selected_feed,
      | 2 => self.selected_favorite,
      | 3 => self.selected_folder,
      | 4 => self.selected_subscription,
      | _ => self.selected_feed
    }
  }

  fn selected_for_tab(
    &mut self
  ) -> &mut usize {
    match self.tab {
      | 0 => &mut self.selected_feed,
      | 2 => {
        &mut self.selected_favorite
      }
      | 3 => &mut self.selected_folder,
      | 4 => {
        &mut self.selected_subscription
      }
      | _ => &mut self.selected_feed
    }
  }

  fn ensure_visible_for_tab(&mut self) {
    let len = self.list_len_for_tab();
    let selected =
      self.selected_value_for_tab();
    let page = self.page_size as usize;
    let offset =
      self.list_offset_for_tab();
    *offset = ensure_offset(
      selected, *offset, page, len
    );
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
        let len = self.feeds_view.len();
        self.selected_feed = move_index(
          self.selected_feed,
          len,
          delta
        );
        self.ensure_visible_for_tab();
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
        self.ensure_visible_for_tab();
      }
      | 3 => {
        let len = self.folders.len();
        self.selected_folder =
          move_index(
            self.selected_folder,
            len,
            delta
          );
        self.ensure_visible_for_tab();
      }
      | _ => {
        let len =
          self.subscriptions_view.len();
        self.selected_subscription =
          move_index(
            self.selected_subscription,
            len,
            delta
          );
        self.ensure_visible_for_tab();
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

fn ensure_offset(
  selected: usize,
  offset: usize,
  page_size: usize,
  len: usize
) -> usize {
  if len == 0 {
    return 0;
  }

  let page = page_size.max(1);
  let mut next =
    offset.min(len.saturating_sub(1));

  if selected < next {
    next = selected;
  } else if selected >= next + page {
    next = selected + 1 - page;
  }

  next.min(len.saturating_sub(1))
}

fn advance_filter_index(
  current: Option<usize>,
  len: usize,
  delta: i32
) -> Option<usize> {
  if len == 0 {
    return None;
  }

  let idx = current
    .map(|v| v as i32)
    .unwrap_or(
      if delta > 0 {
        -1
      } else {
        len as i32
      }
    );

  let next = idx + delta;

  if next < 0 {
    return None;
  }

  if next >= len as i32 {
    return None;
  }

  Some(next as usize)
}
