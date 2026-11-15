use std::thread;

use super::super::super::App;
use super::super::events::AppEvent;
use super::super::helpers::get_json;
use crate::models::{
  FolderFeedRow,
  FolderRow
};

impl App {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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
}
