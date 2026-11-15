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

    let client = self.client.clone();
    thread::spawn(move || {
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

      // A failed entry fetch must not
      // be followed by RefreshAll,
      // which would erase the error and
      // present stale entries as a
      // successful refresh.
      let entries: Option<
        EntryListResponse
      > = if let Some(url) =
        build_entries_url(
          &base_url,
          &entries_mode,
          entries_read_filter,
          entries_page_size,
          entries_offset
        ) {
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

#[cfg(test)]
mod refresh_failure_tests {
  use std::io::{
    BufRead,
    BufReader,
    Write
  };
  use std::net::TcpListener;
  use std::sync::mpsc;
  use std::time::Duration;

  use super::*;
  use crate::app::EntriesMode;
  use crate::config::TuiConfig;

  #[test]
  fn failed_entries_http_does_not_emit_refresh_success()
   {
    let listener =
      TcpListener::bind("127.0.0.1:0")
        .expect(
          "bind disposable HTTP \
           fixture"
        );
    let addr = listener
      .local_addr()
      .expect("fixture address");

    let server =
      thread::spawn(move || {
        for expected in [
          "/v1/feeds ",
          "/v1/subscriptions ",
          "/v1/feeds/counts ",
          "/v1/favorites ",
          "/v1/folders ",
          "/v1/entries?"
        ] {
          let (mut socket, _) =
            listener.accept().expect(
              "accept fixture request"
            );
          socket
            .set_read_timeout(Some(
              Duration::from_secs(5)
            ))
            .expect(
              "set fixture timeout"
            );
          let mut request_line =
            String::new();
          let mut reader =
            BufReader::new(&socket);
          reader
            .read_line(
              &mut request_line
            )
            .expect(
              "read fixture request"
            );
          // Read all headers before
          // closing the socket,
          // avoiding a peer reset from
          // unread request bytes.
          loop {
            let mut header =
              String::new();
            let n = reader
              .read_line(&mut header)
              .expect(
                "read fixture header"
              );
            if n == 0
              || header == "\r\n"
              || header == "\n"
            {
              break;
            }
          }
          drop(reader);
          assert!(
            request_line
              .contains(expected),
            "unexpected request: \
             {request_line}"
          );
          let (status, body) =
            if expected
              == "/v1/entries?"
            {
              (
                "503 Service \
                 Unavailable",
                "entries fixture \
                 unavailable"
              )
            } else {
              ("200 OK", "[]")
            };
          // Keep each HTTP header
          // separate so the
          // narrow rustfmt width cannot
          // split CRLF.
          let headers = [
            format!(
              "HTTP/1.1 {status}\r\n"
            ),
            format!(
              "Content-Length: {}\r\n",
              body.len()
            ),
            "Connection: close\r\n"
              .to_string(),
            "\r\n".to_string()
          ]
          .concat();
          socket
            .write_all(
              headers.as_bytes()
            )
            .expect(
              "serve fixture headers"
            );
          socket
            .write_all(body.as_bytes())
            .expect(
              "serve fixture body"
            );
        }
      });

    let config: TuiConfig =
      toml::from_str(include_str!(
        "../../../../res/config.toml"
      ))
      .expect("fixture config");
    let keys = config
      .resolved_keybindings()
      .expect("fixture keybindings");
    let mut app =
      App::new(&config, keys)
        .expect("offline TUI app");
    app.base_url =
      format!("http://{addr}");
    app.token =
      Some("synthetic-token".into());
    app.entries_mode = EntriesMode::All;

    let (sender, receiver) =
      mpsc::channel();
    app.set_event_sender(sender);
    app.queue_refresh_all();

    match receiver.recv_timeout(
      Duration::from_secs(10)
    ) {
      | Ok(AppEvent::Error {
        message
      }) => {
        assert_eq!(
          message,
          "entries fixture unavailable"
        );
      }
      | _ => {
        panic!(
          "expected HTTP error event"
        );
      }
    }
    assert!(
      receiver
        .recv_timeout(
          Duration::from_millis(250)
        )
        .is_err(),
      "error must not be followed by \
       RefreshAll"
    );
    server
      .join()
      .expect("fixture server");
  }
  #[test]
  fn background_refresh_obeys_configured_timeout()
   {
    let listener =
      TcpListener::bind("127.0.0.1:0")
        .expect(
          "bind local test listener"
        );
    let addr = listener
      .local_addr()
      .expect("test listener address");
    let server =
      thread::spawn(move || {
        let (socket, _) =
          listener.accept().expect(
            "accept background request"
          );
        // Keep the request unanswered
        // longer than the caller's
        // configured timeout.
        thread::sleep(
          Duration::from_millis(1800)
        );
        drop(socket);
      });

    let mut config: TuiConfig =
      toml::from_str(include_str!(
        "../../../../res/config.toml"
      ))
      .expect("parse fixture config");
    config.server.timeout_ms = 120;
    let keys = config
      .resolved_keybindings()
      .expect("parse fixture keys");
    let mut app =
      App::new(&config, keys)
        .expect("create TUI app");
    app.base_url =
      format!("http://{addr}");
    app.token =
      Some("synthetic-token".into());

    let (sender, receiver) =
      mpsc::channel();
    app.set_event_sender(sender);
    app.queue_refresh_all();

    let result = receiver
      .recv_timeout(
        Duration::from_millis(1000)
      )
      .expect(
        "background refresh ignored \
         timeout"
      );
    assert!(matches!(
      result,
      AppEvent::Error { .. }
    ));
    server
      .join()
      .expect("join local test server");
  }
}
