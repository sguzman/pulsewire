use std::thread;

use super::super::super::App;
use super::super::events::AppEvent;
use super::super::helpers::get_json;
use crate::models::{
  EntryDetail,
  FeedDetail
};

impl App {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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
