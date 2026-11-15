use reqwest::Url;
use serde::de::DeserializeOwned;

use super::super::{
  EntriesMode,
  EntriesReadFilter
};

pub(super) fn get_json<
  T: DeserializeOwned
>(
  client: &reqwest::blocking::Client,
  url: &str,
  token: &str
) -> Result<T, String> {
  // Keep the request path, never the
  // URL query or authorization bearer,
  // in client-visible error messages.
  let path = reqwest::Url::parse(url)
    .map(|parsed| {
      parsed.path().to_string()
    })
    .unwrap_or_else(|_| {
      "(invalid URL)".to_string()
    });
  let resp = client
    .get(url)
    .bearer_auth(token)
    .send()
    .map_err(|e| {
      format!("GET {path}: {e}")
    })?;

  if !resp.status().is_success() {
    let status = resp.status();
    let msg = resp
      .text()
      .unwrap_or_else(|_| {
        "request failed".to_string()
      });
    return Err(format!(
      "GET {path} ({status}): {msg}"
    ));
  }

  resp.json::<T>().map_err(|e| {
    format!("GET {path}: {e}")
  })
}

pub(super) fn build_entries_url(
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

#[cfg(test)]
mod get_json_error_tests {
  use std::io::{
    BufRead,
    BufReader,
    Write
  };
  use std::net::TcpListener;
  use std::thread;

  use super::*;

  #[test]
  fn failed_get_json_preserves_path_without_secrets()
   {
    let listener =
      TcpListener::bind("127.0.0.1:0")
        .expect("isolated fixture");
    let addr = listener
      .local_addr()
      .expect("local address");
    let server =
      thread::spawn(move || {
        let (mut socket, _) = listener
          .accept()
          .expect("accept request");
        let mut reader =
          BufReader::new(&socket);
        loop {
          let mut line = String::new();
          let count = reader
            .read_line(&mut line)
            .expect("request header");
          if count == 0
            || line == "\r\n"
            || line == "\n"
          {
            break;
          }
        }
        drop(reader);
        let body =
          r#"{"error":"synthetic"}"#;
        // Build CRLF headers
        // independently: rustfmt line
        // wrapping must not
        // split an HTTP header
        // delimiter.
        let headers = [
          "HTTP/1.1 500 ".to_string(),
          "Internal Server Error\r\n"
            .to_string(),
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
          .write_all(headers.as_bytes())
          .expect(
            "send response headers"
          );
        socket
          .write_all(body.as_bytes())
          .expect("send response body");
      });

    let url = format!(
      "http://{addr}/v1/folders/7/feeds?secret=must-not-print"
    );
    let client =
      reqwest::blocking::Client::new();
    let err = get_json::<
      serde_json::Value
    >(
      &client,
      &url,
      "synthetic-bearer-must-not-print"
    )
    .expect_err("HTTP 500 must fail");
    assert!(err.contains(
      "GET /v1/folders/7/feeds"
    ));
    assert!(err.contains("500"));
    assert!(
      !err.contains("must-not-print")
    );
    server
      .join()
      .expect("fixture HTTP server");
  }
}
