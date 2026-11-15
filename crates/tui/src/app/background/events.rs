use std::collections::HashMap;
use std::sync::mpsc::Sender;

use super::super::App;
use crate::models::{
  EntryDetail,
  EntryListResponse,
  FeedDetail,
  FeedEntryCounts,
  FeedSummary,
  FolderRow
};

pub(crate) enum AppEvent {
  RefreshAll {
    feeds:         Vec<FeedSummary>,
    subscriptions: Vec<String>,
    feed_counts:   Vec<FeedEntryCounts>,
    favorites:     Vec<FeedSummary>,
    folders:       Vec<FolderRow>,
    entries: Option<EntryListResponse>
  },
  RefreshFolders {
    folders: Vec<FolderRow>,
    message: String
  },
  FeedDetail {
    feed_id: String,
    detail:  FeedDetail
  },
  EntryDetail {
    entry_id: i64,
    detail:   EntryDetail
  },
  ToggleSubscribe {
    feed_id:            String,
    desired_subscribed: bool,
    ok:                 bool,
    message:            Option<String>
  },
  ToggleFavorite {
    feed_id:          String,
    desired_favorite: bool,
    ok:               bool,
    message:          Option<String>
  },
  ToggleEntryRead {
    entry_id:     i64,
    desired_read: bool,
    ok:           bool,
    message:      Option<String>
  },
  FolderFeedUpdate {
    feed_id:   String,
    folder_id: i64,
    assigned:  bool,
    ok:        bool,
    message:   Option<String>
  },
  FolderFeeds {
    folder_id: i64,
    feed_ids:  Vec<String>
  },
  Error {
    message: String
  }
}

impl App {
  pub(crate) fn set_event_sender(
    &mut self,
    sender: Sender<AppEvent>
  ) {
    self.event_tx = Some(sender);
  }

  pub(crate) fn apply_event(
    &mut self,
    event: AppEvent
  ) {
    match event {
      | AppEvent::RefreshAll {
        feeds,
        subscriptions,
        feed_counts,
        favorites,
        folders,
        entries
      } => {
        self.feeds = feeds;
        self.subscriptions =
          subscriptions
            .into_iter()
            .collect();
        self.feed_counts = feed_counts
          .into_iter()
          .map(|row| {
            (row.feed_id.clone(), row)
          })
          .collect::<HashMap<_, _>>();
        self.favorites = favorites;
        self.favorite_ids = self
          .favorites
          .iter()
          .map(|row| row.id.clone())
          .collect();
        self.folders = folders;
        if let Some(data) = entries {
          self.entries = data.items;
          self.entries_next_offset =
            data.next_offset;
        }

        self.rebuild_views();
        self.sort_favorites();

        if self.selected_feed
          >= self.feeds_view.len()
        {
          self.selected_feed = 0;
          self.feeds_offset = 0;
        }

        if self.selected_favorite
          >= self.favorites.len()
        {
          self.selected_favorite = 0;
          self.favorites_offset = 0;
        }

        if self.selected_folder
          >= self.folders.len()
        {
          self.selected_folder = 0;
          self.folders_offset = 0;
        }

        if self.selected_subscription
          >= self
            .subscriptions_view
            .len()
        {
          self.selected_subscription =
            0;
          self.subscriptions_offset = 0;
        }

        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);
        self.error = None;

        self.status = format!(
          "Loaded {} feeds ({} shown)",
          self.feeds.len(),
          self.feeds_view.len()
        );
        self.prefetch_selection_details();
        self.request_folder_feeds();
      }
      | AppEvent::RefreshFolders {
        folders,
        message
      } => {
        self.folders = folders;
        if self.selected_folder
          >= self.folders.len()
        {
          self.selected_folder = 0;
          self.folders_offset = 0;
        }
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);
        self.error = None;
        self.status = message;
        self.request_folder_feeds();
      }
      | AppEvent::FeedDetail {
        feed_id,
        detail
      } => {
        self.feed_details.insert(
          feed_id,
          detail
        );
      }
      | AppEvent::EntryDetail {
        entry_id,
        detail
      } => {
        self.entry_details.insert(
          entry_id,
          detail
        );
      }
      | AppEvent::ToggleSubscribe {
        feed_id,
        desired_subscribed,
        ok,
        message
      } => {
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);

        if ok {
          self.status =
            if desired_subscribed {
              format!(
                "Subscribed to \
                 {feed_id}"
              )
            } else {
              format!(
                "Unsubscribed from \
                 {feed_id}"
              )
            };
          self.error = None;
        } else {
          if desired_subscribed {
            self
              .subscriptions
              .remove(&feed_id);
          } else {
            self
              .subscriptions
              .insert(feed_id.clone());
          }
          self.rebuild_views();
          self.error = Some(
            message.unwrap_or_else(
              || {
                "subscription update \
                 failed"
                  .to_string()
              }
            )
          );
        }
      }
      | AppEvent::ToggleFavorite {
        feed_id,
        desired_favorite,
        ok,
        message
      } => {
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);

        if ok {
          self.status = if desired_favorite {
            format!(
              "Favorited {feed_id}"
            )
          } else {
            format!(
              "Unfavorited {feed_id}"
            )
          };
          self.error = None;
        } else {
          if desired_favorite {
            self.favorite_ids
              .remove(&feed_id);
            self.favorites
              .retain(|row| {
                row.id != feed_id
              });
          } else {
            self.favorite_ids
              .insert(feed_id.clone());
            if let Some(feed) =
              self.feeds.iter().find(
                |row| row.id == feed_id
              )
            {
              self.favorites.push(feed.clone());
              self.sort_favorites();
            }
          }
          self.error = Some(
            message.unwrap_or_else(
              || {
                "favorite update failed"
                  .to_string()
              }
            )
          );
        }
      }
      | AppEvent::ToggleEntryRead {
        entry_id,
        desired_read,
        ok,
        message
      } => {
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);

        if ok {
          self.status = if desired_read
          {
            "Marked read".to_string()
          } else {
            "Marked unread".to_string()
          };
          self.error = None;
        } else {
          if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|row| {
              row.id == entry_id
            })
          {
            entry.is_read =
              !desired_read;
          }
          self.error = Some(
            message.unwrap_or_else(
              || {
                "toggle read failed"
                  .to_string()
              }
            )
          );
        }
      }
      | AppEvent::FolderFeedUpdate {
        feed_id,
        folder_id,
        assigned,
        ok,
        message
      } => {
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);

        if ok {
          self.status = if assigned {
            format!(
              "Added {feed_id} to folder \
               #{folder_id}"
            )
          } else {
            format!(
              "Removed {feed_id} from \
               folder #{folder_id}"
            )
          };
          self.error = None;
        } else {
          self.error = Some(
            message.unwrap_or_else(
              || {
                "folder update failed"
                  .to_string()
              }
            )
          );
        }
      }
      | AppEvent::FolderFeeds {
        folder_id,
        feed_ids
      } => {
        // This response completes a queued request even
        // when the user has selected another folder.
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);
        self.loading =
          self.pending_requests > 0;

        let selected = self
          .folders
          .get(self.selected_folder)
          .map(|folder| folder.id);
        if selected == Some(folder_id) {
          self.update_folder_feeds(feed_ids);
          self.status = format!(
            "Loaded {} feeds for folder #{folder_id}",
            self.folder_feeds.len()
          );
        }
      }
      | AppEvent::Error {
        message
      } => {
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);
        self.error = Some(message);
      }
    }
  }
}

#[cfg(test)]
mod async_refresh_state_tests {
  use super::*;
  use crate::config::TuiConfig;

  fn test_app() -> App {
    let config: TuiConfig =
      toml::from_str(include_str!(
        "../../../res/config.toml"
      ))
      .expect("parse fixture config");
    let keys = config
      .resolved_keybindings()
      .expect("resolve fixture keys");
    App::new(&config, keys)
      .expect("create offline TUI")
  }

  #[test]
  fn folder_feed_response_releases_pending_request() {
    let mut app = test_app();
    app.folders.push(FolderRow {
      id: 42,
      name: "News".to_string()
    });
    app.pending_requests = 2;
    app.loading = true;

    app.apply_event(AppEvent::FolderFeeds {
      folder_id: 42,
      feed_ids: vec!["fixture".to_string()]
    });
    assert_eq!(app.pending_requests, 1);
    assert!(app.loading);
    assert_eq!(
      app.folder_feed_ids,
      vec!["fixture".to_string()]
    );

    // A stale response still frees its request slot,
    // without overwriting the currently selected folder.
    app.apply_event(AppEvent::FolderFeeds {
      folder_id: 99,
      feed_ids: vec!["stale".to_string()]
    });
    assert_eq!(app.pending_requests, 0);
    assert!(!app.loading);
    assert_eq!(
      app.folder_feed_ids,
      vec!["fixture".to_string()]
    );
  }

  #[test]
  fn failed_refresh_preserves_visible_error() {
    let mut app = test_app();
    app.pending_requests = 1;
    app.loading = true;
    app.apply_event(AppEvent::Error {
      message: "temporary HTTP failure".into()
    });

    assert_eq!(app.pending_requests, 0);
    assert!(!app.loading);
    assert_eq!(
      app.error.as_deref(),
      Some("temporary HTTP failure")
    );
  }
}
