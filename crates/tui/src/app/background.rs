use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::thread;

use reqwest::Url;
use serde::de::DeserializeOwned;

use super::{
  App,
  EntriesMode,
  EntriesReadFilter
};
use crate::models::{
  EntryDetail,
  EntryListResponse,
  FeedDetail,
  FeedEntryCounts,
  FeedSummary,
  FolderFeedRow,
  FolderRow,
  SubscriptionRow
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
  RefreshFavorites {
    favorites: Vec<FeedSummary>,
    message:   String
  },
  RefreshEntries {
    entries: EntryListResponse,
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
      | AppEvent::RefreshFavorites {
        favorites,
        message
      } => {
        self.favorites = favorites;
        self.favorite_ids = self
          .favorites
          .iter()
          .map(|row| row.id.clone())
          .collect();
        self.sort_favorites();
        if self.selected_favorite
          >= self.favorites.len()
        {
          self.selected_favorite = 0;
          self.favorites_offset = 0;
        }
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);
        self.error = None;
        self.status = message;
      }
      | AppEvent::RefreshEntries {
        entries,
        message
      } => {
        self.entries = entries.items;
        self.entries_next_offset =
          entries.next_offset;
        if self.selected_entry
          >= self.entries.len()
        {
          self.selected_entry = 0;
        }
        self.loading = false;
        self.pending_requests = self
          .pending_requests
          .saturating_sub(1);
        self.error = None;
        self.status = message;
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

  pub(crate) fn queue_refresh_all(
    &mut self
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();
    let entries_mode =
      self.entries_mode.clone();
    let entries_read_filter =
      self.entries_read_filter;
    let entries_page_size =
      self.entries_page_size;
    let entries_offset =
      self.entries_offset;

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );

      let feeds: Vec<FeedSummary> =
        match get_json(
          &client,
          &format!(
            "{base_url}/v1/feeds"
          ),
          &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let subs: Vec<SubscriptionRow> =
        match get_json(
          &client,
          &format!(
            "{base_url}/v1/\
             subscriptions"
          ),
          &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let feed_counts: Vec<
        FeedEntryCounts
      > = match get_json(
        &client,
        &format!(
          "{base_url}/v1/feeds/counts"
        ),
        &token
      ) {
        | Ok(data) => data,
        | Err(err) => {
          let _ = sender.send(
            AppEvent::Error {
              message: err
            }
          );
          return;
        }
      };

      let favorites: Vec<FeedSummary> =
        match get_json(
          &client,
          &format!(
            "{base_url}/v1/favorites"
          ),
          &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let folders: Vec<FolderRow> =
        match get_json(
          &client,
          &format!(
            "{base_url}/v1/folders"
          ),
          &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let entries = build_entries_url(
        &base_url,
        &entries_mode,
        entries_read_filter,
        entries_page_size,
        entries_offset
      )
      .and_then(|url| {
        match get_json(
          &client, &url, &token
        ) {
          | Ok(data) => Some(data),
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            None
          }
        }
      });

      let subscription_ids = subs
        .into_iter()
        .map(|row| row.feed_id)
        .collect();

      let _ = sender.send(
        AppEvent::RefreshAll {
          feeds,
          subscriptions:
            subscription_ids,
          feed_counts,
          favorites,
          folders,
          entries
        }
      );
    });
  }

  pub(crate) fn queue_refresh_folders(
    &mut self,
    message: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let folders: Vec<FolderRow> =
        match get_json(
          &client,
          &format!(
            "{base_url}/v1/folders"
          ),
          &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let _ = sender.send(
        AppEvent::RefreshFolders {
          folders,
          message
        }
      );
    });
  }

  pub(crate) fn queue_refresh_favorites(
    &mut self,
    message: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let favorites: Vec<FeedSummary> =
        match get_json(
          &client,
          &format!(
            "{base_url}/v1/favorites"
          ),
          &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let _ = sender.send(
        AppEvent::RefreshFavorites {
          favorites,
          message
        }
      );
    });
  }

  pub(crate) fn queue_refresh_entries(
    &mut self,
    message: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();
    let entries_mode =
      self.entries_mode.clone();
    let entries_read_filter =
      self.entries_read_filter;
    let entries_page_size =
      self.entries_page_size;
    let entries_offset =
      self.entries_offset;

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = match build_entries_url(
        &base_url,
        &entries_mode,
        entries_read_filter,
        entries_page_size,
        entries_offset
      ) {
        | Some(url) => url,
        | None => {
          let _ = sender.send(
            AppEvent::Error {
              message:
                "no entries source \
                 selected"
                  .to_string()
            }
          );
          return;
        }
      };

      let entries: EntryListResponse =
        match get_json(
          &client, &url, &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let _ = sender.send(
        AppEvent::RefreshEntries {
          entries,
          message
        }
      );
    });
  }

  pub(crate) fn queue_toggle_subscribe(
    &mut self,
    feed_id: String,
    desired_subscribed: bool
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let result = if desired_subscribed
      {
        let url = format!(
          "{base_url}/v1/subscriptions"
        );
        let body = serde_json::json!({
          "feed_id": feed_id
        });
        client
          .post(url)
          .bearer_auth(&token)
          .json(&body)
          .send()
      } else {
        let url = format!(
          "{base_url}/v1/\
           subscriptions/{feed_id}"
        );
        client
          .delete(url)
          .bearer_auth(&token)
          .send()
      };

      match result {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let _ = sender.send(
            AppEvent::ToggleSubscribe {
              feed_id,
              desired_subscribed,
              ok: true,
              message: None
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "subscription update \
               failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::ToggleSubscribe {
              feed_id,
              desired_subscribed,
              ok: false,
              message: Some(message)
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::ToggleSubscribe {
              feed_id,
              desired_subscribed,
              ok: false,
              message: Some(
                err.to_string()
              )
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_toggle_favorite(
    &mut self,
    feed_id: String,
    desired_favorite: bool
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let result = if desired_favorite {
        let url = format!(
          "{base_url}/v1/favorites"
        );
        let body = serde_json::json!({
          "feed_id": feed_id
        });
        client
          .post(url)
          .bearer_auth(&token)
          .json(&body)
          .send()
      } else {
        let url = format!(
          "{base_url}/v1/favorites/\
           {feed_id}"
        );
        client
          .delete(url)
          .bearer_auth(&token)
          .send()
      };

      match result {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let _ = sender.send(
            AppEvent::ToggleFavorite {
              feed_id,
              desired_favorite,
              ok: true,
              message: None
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "favorite update failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::ToggleFavorite {
              feed_id,
              desired_favorite,
              ok: false,
              message: Some(message)
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::ToggleFavorite {
              feed_id,
              desired_favorite,
              ok: false,
              message: Some(
                err.to_string()
              )
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_toggle_entry_read(
    &mut self,
    entry_id: i64,
    desired_read: bool
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/entries/\
         {entry_id}/read"
      );

      let req = if desired_read {
        client.post(url)
      } else {
        client.delete(url)
      };

      match req
        .bearer_auth(&token)
        .send()
      {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let _ = sender.send(
            AppEvent::ToggleEntryRead {
              entry_id,
              desired_read,
              ok: true,
              message: None
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "toggle read failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::ToggleEntryRead {
              entry_id,
              desired_read,
              ok: false,
              message: Some(message)
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::ToggleEntryRead {
              entry_id,
              desired_read,
              ok: false,
              message: Some(
                err.to_string()
              )
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_create_folder(
    &mut self,
    name: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/folders"
      );
      let body = serde_json::json!({
        "name": name
      });

      let resp = client
        .post(url)
        .bearer_auth(&token)
        .json(&body)
        .send();

      match resp {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let folders: Vec<FolderRow> =
            match get_json(
              &client,
              &format!(
                "{base_url}/v1/folders"
              ),
              &token
            ) {
              | Ok(data) => data,
              | Err(err) => {
                let _ = sender.send(
                  AppEvent::Error {
                    message: err
                  }
                );
                return;
              }
            };

          let _ = sender.send(
            AppEvent::RefreshFolders {
              folders,
              message: "Folder created"
                .to_string()
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "folder create failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::Error {
              message
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::Error {
              message: err.to_string()
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_rename_folder(
    &mut self,
    folder_id: i64,
    name: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/folders/\
         {folder_id}"
      );
      let body = serde_json::json!({
        "name": name
      });

      let resp = client
        .patch(url)
        .bearer_auth(&token)
        .json(&body)
        .send();

      match resp {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let folders: Vec<FolderRow> =
            match get_json(
              &client,
              &format!(
                "{base_url}/v1/folders"
              ),
              &token
            ) {
              | Ok(data) => data,
              | Err(err) => {
                let _ = sender.send(
                  AppEvent::Error {
                    message: err
                  }
                );
                return;
              }
            };

          let _ = sender.send(
            AppEvent::RefreshFolders {
              folders,
              message: "Folder renamed"
                .to_string()
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "folder rename failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::Error {
              message
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::Error {
              message: err.to_string()
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_delete_folder(
    &mut self,
    folder_id: i64
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/folders/\
         {folder_id}"
      );

      let resp = client
        .delete(url)
        .bearer_auth(&token)
        .send();

      match resp {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let folders: Vec<FolderRow> =
            match get_json(
              &client,
              &format!(
                "{base_url}/v1/folders"
              ),
              &token
            ) {
              | Ok(data) => data,
              | Err(err) => {
                let _ = sender.send(
                  AppEvent::Error {
                    message: err
                  }
                );
                return;
              }
            };

          let _ = sender.send(
            AppEvent::RefreshFolders {
              folders,
              message: "Folder deleted"
                .to_string()
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "folder delete failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::Error {
              message
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::Error {
              message: err.to_string()
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_folder_feeds(
    &mut self,
    folder_id: i64
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/folders/\
         {folder_id}/feeds"
      );

      let feeds: Vec<FolderFeedRow> =
        match get_json(
          &client, &url, &token
        ) {
          | Ok(data) => data,
          | Err(err) => {
            let _ = sender.send(
              AppEvent::Error {
                message: err
              }
            );
            return;
          }
        };

      let feed_ids = feeds
        .into_iter()
        .map(|row| row.feed_id)
        .collect();

      let _ = sender.send(
        AppEvent::FolderFeeds {
          folder_id,
          feed_ids
        }
      );
    });
  }

  pub(crate) fn queue_assign_folder_feed(
    &mut self,
    folder_id: i64,
    feed_id: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/folders/\
         {folder_id}/feeds"
      );
      let body = serde_json::json!({
        "feed_id": feed_id
      });

      let resp = client
        .post(url)
        .bearer_auth(&token)
        .json(&body)
        .send();

      match resp {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let _ = sender.send(
            AppEvent::FolderFeedUpdate {
              feed_id,
              folder_id,
              assigned: true,
              ok: true,
              message: None
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "folder add failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::FolderFeedUpdate {
              feed_id,
              folder_id,
              assigned: true,
              ok: false,
              message: Some(message)
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::FolderFeedUpdate {
              feed_id,
              folder_id,
              assigned: true,
              ok: false,
              message: Some(
                err.to_string()
              )
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_unassign_folder_feed(
    &mut self,
    folder_id: i64,
    feed_id: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    self.pending_requests += 1;
    self.loading = true;

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/folders/\
         {folder_id}/feeds/{feed_id}"
      );

      let resp = client
        .delete(url)
        .bearer_auth(&token)
        .send();

      match resp {
        | Ok(resp)
          if resp
            .status()
            .is_success() =>
        {
          let _ = sender.send(
            AppEvent::FolderFeedUpdate {
              feed_id,
              folder_id,
              assigned: false,
              ok: true,
              message: None
            }
          );
        }
        | Ok(resp) => {
          let message = resp
            .text()
            .unwrap_or_else(|_| {
              "folder remove failed"
                .to_string()
            });
          let _ = sender.send(
            AppEvent::FolderFeedUpdate {
              feed_id,
              folder_id,
              assigned: false,
              ok: false,
              message: Some(message)
            }
          );
        }
        | Err(err) => {
          let _ = sender.send(
            AppEvent::FolderFeedUpdate {
              feed_id,
              folder_id,
              assigned: false,
              ok: false,
              message: Some(
                err.to_string()
              )
            }
          );
        }
      }
    });
  }

  pub(crate) fn queue_feed_detail(
    &mut self,
    feed_id: String
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/feeds/{feed_id}"
      );
      match get_json::<FeedDetail>(
        &client, &url, &token
      ) {
        | Ok(detail) => {
          let _ = sender.send(
            AppEvent::FeedDetail {
              feed_id,
              detail
            }
          );
        }
        | Err(_) => {}
      }
    });
  }

  pub(crate) fn queue_entry_detail(
    &mut self,
    entry_id: i64
  ) {
    let Some(sender) =
      self.event_tx.clone()
    else {
      return;
    };

    let Some(token) =
      self.token.clone()
    else {
      return;
    };

    let base_url =
      self.base_url.clone();

    thread::spawn(move || {
      let client =
        reqwest::blocking::Client::new(
        );
      let url = format!(
        "{base_url}/v1/entries/\
         {entry_id}"
      );
      match get_json::<EntryDetail>(
        &client, &url, &token
      ) {
        | Ok(detail) => {
          let _ = sender.send(
            AppEvent::EntryDetail {
              entry_id,
              detail
            }
          );
        }
        | Err(_) => {}
      }
    });
  }
}

fn get_json<T: DeserializeOwned>(
  client: &reqwest::blocking::Client,
  url: &str,
  token: &str
) -> Result<T, String> {
  let resp = client
    .get(url)
    .bearer_auth(token)
    .send()
    .map_err(|e| e.to_string())?;

  if !resp.status().is_success() {
    let msg = resp
      .text()
      .unwrap_or_else(|_| {
        "request failed".to_string()
      });
    return Err(msg);
  }

  resp
    .json::<T>()
    .map_err(|e| e.to_string())
}

fn build_entries_url(
  base_url: &str,
  mode: &EntriesMode,
  read_filter: EntriesReadFilter,
  limit: u32,
  offset: i64
) -> Option<String> {
  let (path, feed_id, query) =
    match mode {
      | EntriesMode::None => {
        return None
      }
      | EntriesMode::Feed(feed_id) => {
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
        &limit.to_string()
      )
      .append_pair(
        "offset",
        &offset.to_string()
      );

    if let Some(feed_id) = feed_id {
      pairs.append_pair(
        "feed_id", &feed_id
      );
    }

    if let Some(query) = query {
      pairs.append_pair("q", &query);
    }

    match read_filter {
      | EntriesReadFilter::All => {}
      | EntriesReadFilter::Read => {
        pairs
          .append_pair("read", "read");
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
