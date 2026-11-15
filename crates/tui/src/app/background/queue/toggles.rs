use std::thread;

use super::super::super::App;
use super::super::events::AppEvent;

impl App {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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

    let client = self.client.clone();
    thread::spawn(move || {
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
}
