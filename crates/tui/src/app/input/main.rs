use anyhow::Result;
use crossterm::event::{
  KeyCode,
  KeyEvent,
  KeyModifiers
};

use super::super::{
  App,
  EntriesReadFilter
};

impl App {
  pub(super) fn handle_main_key(
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

    if self.input.is_some() {
      return self.handle_input_key(key);
    }

    if self.modal.is_some() {
      return self.handle_modal_key(key);
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
      self.request_folder_feeds();
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
      if self.tab == 3 {
        self.request_folder_feeds();
      }
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.next_tab,
      key
    ) {
      self.tab = (self.tab + 1) % 5;
      if self.tab == 3 {
        self.request_folder_feeds();
      }
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
      &self.keys.go_top,
      key
    ) {
      self.jump_top();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.go_middle,
      key
    ) {
      self.jump_middle();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.go_bottom,
      key
    ) {
      self.jump_bottom();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.open_category_menu,
      key
    ) {
      self.open_category_menu();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.open_tag_menu,
      key
    ) {
      self.open_tag_menu();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.open_sort_menu,
      key
    ) {
      self.open_sort_menu();
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
      &self.keys.toggle_hide_empty,
      key
    ) {
      self.hide_empty_feeds =
        !self.hide_empty_feeds;
      self.rebuild_views();
      self.status = format!(
        "Hide empty feeds: {}",
        self.hide_empty_feeds
      );
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.toggle_hide_read,
      key
    ) {
      self.hide_read_feeds =
        !self.hide_read_feeds;
      self.rebuild_views();
      self.status = format!(
        "Hide read feeds: {}",
        self.hide_read_feeds
      );
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
      &self.keys.open_all_entries,
      key
    ) {
      self.open_all_entries()?;
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.open_search,
      key
    ) {
      self.open_search_input();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.toggle_entries_filter,
      key
    ) {
      self.entries_read_filter =
        match self.entries_read_filter {
          | EntriesReadFilter::All => {
            EntriesReadFilter::Unread
          }
          | EntriesReadFilter::Unread => {
            EntriesReadFilter::Read
          }
          | EntriesReadFilter::Read => {
            EntriesReadFilter::All
          }
        };
      self.entries_offset = 0;
      if self.tab == 1 {
        self.refresh_entries()?;
      }
      self.status = format!(
        "Entries filter: {}",
        self.entries_filter_label()
      );
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.toggle_read,
      key
    ) {
      self.trigger_toggle_entry_read();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.toggle_subscribe,
      key
    ) {
      self.trigger_toggle_subscribe();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.toggle_favorite,
      key
    ) {
      self.trigger_toggle_favorite();
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.folder_create,
      key
    ) {
      if self.tab == 3 {
        self.open_folder_create_input();
      }
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.folder_rename,
      key
    ) {
      if self.tab == 3 {
        self.open_folder_rename_input();
      }
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.folder_delete,
      key
    ) {
      if self.tab == 3 {
        if let Some(folder) = self
          .folders
          .get(self.selected_folder)
          .cloned()
        {
          self.queue_delete_folder(
            folder.id
          );
        } else {
          self.status =
            "No folder selected"
              .to_string();
        }
      }
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.folder_assign,
      key
    ) {
      if let Some(feed) =
        self.current_feed_context()
      {
        self.open_folder_menu(
          feed.id, true
        );
      } else {
        self.status =
          "No feed selected"
            .to_string();
      }
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.folder_unassign,
      key
    ) {
      if let Some(feed) =
        self.current_feed_context()
      {
        self.open_folder_menu(
          feed.id, false
        );
      } else {
        self.status =
          "No feed selected"
            .to_string();
      }
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

    if self.key_matches(
      &self.keys.feeds_next,
      key
    ) {
      if self.tab == 0 {
        self.next_list_page();
      }
      return Ok(false);
    }

    if self.key_matches(
      &self.keys.feeds_prev,
      key
    ) {
      if self.tab == 0 {
        self.prev_list_page();
      }
      return Ok(false);
    }

    Ok(false)
  }

  fn trigger_toggle_entry_read(
    &mut self
  ) {
    if self.tab != 1 {
      return;
    }

    let entry = match self
      .entries
      .get(self.selected_entry)
    {
      | Some(entry) => entry.clone(),
      | None => return
    };

    if let Some(row) = self
      .entries
      .get_mut(self.selected_entry)
    {
      row.is_read = !entry.is_read;
    }

    if let Some(detail) = self
      .entry_details
      .get_mut(&entry.id)
    {
      detail.is_read = !entry.is_read;
    }

    self.status = if entry.is_read {
      "Marked unread (pending)"
        .to_string()
    } else {
      "Marked read (pending)"
        .to_string()
    };

    self.queue_toggle_entry_read(
      entry.id,
      !entry.is_read
    );
  }

  fn trigger_toggle_subscribe(
    &mut self
  ) {
    if self.tab != 0
      && self.tab != 2
      && self.tab != 4
    {
      return;
    }

    let feed = match self
      .current_feed_context()
    {
      | Some(feed) => feed,
      | None => return
    };

    let subscribed = self
      .subscriptions
      .contains(&feed.id);
    let desired = !subscribed;

    if desired {
      self
        .subscriptions
        .insert(feed.id.clone());
      self.status = "Subscribed \
                     (pending)"
        .to_string();
    } else {
      self
        .subscriptions
        .remove(&feed.id);
      self.status = "Unsubscribed \
                     (pending)"
        .to_string();
    }

    self.rebuild_views();
    self.queue_toggle_subscribe(
      feed.id, desired
    );
  }

  fn trigger_toggle_favorite(
    &mut self
  ) {
    if self.tab != 0
      && self.tab != 2
      && self.tab != 4
    {
      return;
    }

    let feed = match self
      .current_feed_context()
    {
      | Some(feed) => feed,
      | None => return
    };

    let is_favorite = self
      .favorite_ids
      .contains(&feed.id);
    let desired = !is_favorite;

    if desired {
      self
        .favorite_ids
        .insert(feed.id.clone());
      self.favorites.push(feed.clone());
      self.sort_favorites();
      self.status =
        "Favorited (pending)"
          .to_string();
    } else {
      self
        .favorite_ids
        .remove(&feed.id);
      self.favorites.retain(|row| {
        row.id != feed.id
      });
      self.status = "Unfavorited \
                     (pending)"
        .to_string();
      if self.selected_favorite
        >= self.favorites.len()
      {
        self.selected_favorite = 0;
        self.favorites_offset = 0;
      }
    }

    self.queue_toggle_favorite(
      feed.id, desired
    );
  }
}
