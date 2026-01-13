use anyhow::{
  Context,
  Result
};
use reqwest::Url;

use super::super::{
  App,
  EntriesMode,
  EntriesReadFilter
};
use crate::models::EntryListResponse;

impl App {
  pub(crate) fn refresh_entries(
    &mut self
  ) -> Result<()> {
    let Some(url) = self.entries_url()
    else {
      self.status =
        "Select a feed, folder, or \
         search to load entries."
          .to_string();
      return Ok(());
    };

    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

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

    if let Some((current, total)) =
      self.entries_page_info()
    {
      self.status = format!(
        "Loaded {} entries (page \
         {current}/{total}, offset {})",
        self.entries.len(),
        self.entries_offset
      );
    } else {
      self.status = format!(
        "Loaded {} entries (offset {})",
        self.entries.len(),
        self.entries_offset
      );
    }

    Ok(())
  }

  pub(crate) fn open_entries(
    &mut self
  ) -> Result<()> {
    match self.tab {
      | 0 => {
        let feed = self
          .feeds_view
          .get(self.selected_feed)
          .and_then(|idx| {
            self.feeds.get(*idx)
          })
          .cloned();

        if let Some(feed) = feed {
          self.entries_mode =
            EntriesMode::Feed(feed.id);
        } else {
          self.status =
            "No feed selected"
              .to_string();
          return Ok(());
        }
      }
      | 2 => {
        let feed = self
          .favorites
          .get(self.selected_favorite)
          .cloned();
        if let Some(feed) = feed {
          self.entries_mode =
            EntriesMode::Feed(feed.id);
        } else {
          self.status = "No favorite \
                         selected"
            .to_string();
          return Ok(());
        }
      }
      | 3 => {
        let folder = self
          .folders
          .get(self.selected_folder)
          .cloned();
        if let Some(folder) = folder {
          self.entries_mode =
            EntriesMode::Folder(
              folder.id
            );
        } else {
          self.status =
            "No folder selected"
              .to_string();
          return Ok(());
        }
      }
      | 4 => {
        let feed = self
          .subscriptions_view
          .get(
            self.selected_subscription
          )
          .and_then(|idx| {
            self.feeds.get(*idx)
          })
          .cloned();
        if let Some(feed) = feed {
          self.entries_mode =
            EntriesMode::Feed(feed.id);
        } else {
          self.status =
            "No subscription selected"
              .to_string();
          return Ok(());
        }
      }
      | _ => {}
    }

    self.entries_offset = 0;
    self.selected_entry = 0;
    self.tab = 1;
    self.refresh_entries()?;

    Ok(())
  }

  pub(crate) fn open_all_entries(
    &mut self
  ) -> Result<()> {
    self.entries_mode =
      EntriesMode::All;
    self.entries_offset = 0;
    self.selected_entry = 0;
    self.tab = 1;
    self.refresh_entries()?;
    Ok(())
  }

  pub(crate) fn open_search_entries(
    &mut self,
    query: String
  ) -> Result<()> {
    if query.trim().is_empty() {
      self.status = "Search query is \
                     empty"
        .to_string();
      return Ok(());
    }

    let feed_id = self
      .current_feed_context()
      .map(|feed| feed.id);

    self.entries_mode =
      EntriesMode::Search {
        query,
        feed_id
      };
    self.entries_offset = 0;
    self.selected_entry = 0;
    self.tab = 1;
    self.refresh_entries()?;
    Ok(())
  }

  pub(crate) fn next_entries_page(
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

  pub(crate) fn prev_entries_page(
    &mut self
  ) -> Result<()> {
    if self.tab != 1 {
      return Ok(());
    }

    if self.entries_offset == 0 {
      return Ok(());
    }

    let size =
      self.entries_page_size as i64;
    self.entries_offset =
      (self.entries_offset - size)
        .max(0);
    self.refresh_entries()?;

    Ok(())
  }

  pub(crate) fn entries_url(
    &self
  ) -> Option<String> {
    let base_url = &self.base_url;
    let (path, feed_id, query) =
      match &self.entries_mode {
        | EntriesMode::None => {
          return None
        }
        | EntriesMode::Feed(
          feed_id
        ) => {
          (
            format!(
              "{base_url}/v1/feeds/\
               {feed_id}/entries"
            ),
            None,
            None
          )
        }
        | EntriesMode::Folder(
          folder_id
        ) => {
          (
            format!(
              "{base_url}/v1/folders/\
               {folder_id}/entries"
            ),
            None,
            None
          )
        }
        | EntriesMode::All => {
          (
            format!(
              "{base_url}/v1/entries"
            ),
            None,
            None
          )
        }
        | EntriesMode::Search {
          query,
          feed_id
        } => {
          (
            format!(
              "{base_url}/v1/entries/\
               search"
            ),
            feed_id.clone(),
            Some(query.clone())
          )
        }
      };

    let mut url =
      Url::parse(&path).ok()?;
    {
      let mut pairs =
        url.query_pairs_mut();
      pairs
        .append_pair(
          "limit",
          &self
            .entries_page_size
            .to_string()
        )
        .append_pair(
          "offset",
          &self
            .entries_offset
            .to_string()
        );

      if let Some(feed_id) = feed_id {
        pairs.append_pair(
          "feed_id", &feed_id
        );
      }

      if let Some(query) = query {
        pairs.append_pair("q", &query);
      }

      match self.entries_read_filter {
        | EntriesReadFilter::All => {}
        | EntriesReadFilter::Read => {
          pairs.append_pair(
            "read", "read"
          );
        }
        | EntriesReadFilter::Unread => {
          pairs.append_pair(
            "read", "unread"
          );
        }
      }
    }

    Some(url.to_string())
  }

  pub(crate) fn current_feed_context(
    &self
  ) -> Option<crate::models::FeedSummary>
  {
    match self.tab {
      | 0 => {
        self
          .feeds_view
          .get(self.selected_feed)
          .and_then(|idx| {
            self.feeds.get(*idx)
          })
          .cloned()
      }
      | 2 => {
        self
          .favorites
          .get(self.selected_favorite)
          .cloned()
      }
      | 4 => {
        self
          .subscriptions_view
          .get(
            self.selected_subscription
          )
          .and_then(|idx| {
            self.feeds.get(*idx)
          })
          .cloned()
      }
      | 1 => {
        match &self.entries_mode {
          | EntriesMode::Feed(
            feed_id
          ) => {
            self
              .feeds
              .iter()
              .find(|feed| {
                feed.id == *feed_id
              })
              .cloned()
          }
          | _ => None
        }
      }
      | _ => None
    }
  }
}
