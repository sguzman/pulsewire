use std::thread;

use super::super::super::App;
use super::super::events::AppEvent;
use super::super::helpers::{
  build_entries_url,
  get_json
};
use crate::models::{
  EntryListResponse,
  FeedEntryCounts,
  FeedSummary,
  FolderRow,
  SubscriptionRow
};

impl App {
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

      // A failed entry fetch must not be followed by
      // RefreshAll, which would erase the error and
      // present stale entries as a successful refresh.
      let entries: Option<
        EntryListResponse
      > = if let Some(url) =
        build_entries_url(
          &base_url,
          &entries_mode,
          entries_read_filter,
          entries_page_size,
          entries_offset
        )
      {
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
            return;
          }
        }
      } else {
        None
      };

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
}
