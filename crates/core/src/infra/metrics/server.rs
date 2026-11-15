use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{
  AsyncReadExt,
  AsyncWriteExt
};
use tokio::net::TcpListener;

use super::Metrics;

pub(super) async fn spawn(
  bind: &str,
  metrics: Arc<Metrics>
) -> Result<SocketAddr, String> {
  let addr: SocketAddr =
    bind.parse().map_err(|e| {
      format!(
        "invalid metrics bind '{}': \
         {e}",
        bind
      )
    })?;

  let listener =
    TcpListener::bind(addr)
      .await
      .map_err(|e| {
        format!(
          "failed to bind metrics \
           server on {}: {e}",
          bind
        )
      })?;

  let bound_addr = listener
    .local_addr()
    .map_err(|e| {
      format!(
        "failed to determine metrics listener address: {e}"
      )
    })?;

  tokio::spawn(async move {
    loop {
      let (mut stream, _) =
        match listener.accept().await {
          | Ok(pair) => pair,
          | Err(_) => continue
        };

      let metrics = metrics.clone();

      tokio::spawn(async move {
        let mut buf = [0u8; 8192];

        let n = match stream
          .read(&mut buf)
          .await
        {
          | Ok(n) => n,
          | Err(_) => return
        };

        let req =
          String::from_utf8_lossy(
            &buf[..n]
          );

        let path = req
          .lines()
          .next()
          .and_then(|line| {
            line
              .split_whitespace()
              .nth(1)
          })
          .unwrap_or("/");

        let (status, body) =
          if path == "/metrics" {
            ("200 OK", metrics.render())
          } else {
            (
              "404 Not Found",
              "not found\n".to_string()
            )
          };

        let mut resp =
          format!("HTTP/1.1 {status}\r\n");
        resp.push_str(
          "Content-Type: text/plain; \
           version=0.0.4\r\n"
        );
        resp.push_str(&format!(
          "Content-Length: {}\r\n",
          body.len()
        ));
        resp.push_str(
          "Connection: close\r\n\r\n"
        );
        resp.push_str(&body);

        let _ = stream
          .write_all(resp.as_bytes())
          .await;
      });
    }
  });

  Ok(bound_addr)
}

#[cfg(test)]
mod metrics_http_tests {
  use super::*;
  use tokio::net::TcpStream;

  async fn request(
    addr: SocketAddr,
    path: &str
  ) -> String {
    let mut stream =
      TcpStream::connect(addr)
        .await
        .expect("connect metrics listener");
    stream
      .write_all(format!(
        "GET {path} HTTP/1.1\r\n\
         Host: localhost\r\n\r\n"
      ).as_bytes())
      .await
      .expect("send metrics request");

    let mut bytes = Vec::new();
    stream
      .read_to_end(&mut bytes)
      .await
      .expect("receive metrics response");
    String::from_utf8(bytes)
      .expect("UTF-8 metrics reply")
  }

  fn assert_http_response(
    response: &str,
    status: &str,
    body_expected: &str
  ) {
    let (headers, body) = response
      .split_once("\r\n\r\n")
      .expect("HTTP CRLF header boundary");
    assert_eq!(
      headers.split("\r\n").next(),
      Some(status)
    );
    assert!(headers.contains(
      "\r\nContent-Type: text/plain; version=0.0.4"
    ));
    let length = headers
      .split("\r\n")
      .find_map(|line| line.strip_prefix(
        "Content-Length: "
      ))
      .expect("Content-Length")
      .parse::<usize>()
      .expect("numeric Content-Length");
    assert_eq!(length, body.len());
    assert_eq!(body, body_expected);
  }

  #[tokio::test]
  async fn metrics_http_uses_valid_crlf_and_content_length()
   {
    let metrics =
      Arc::new(Metrics::new(&[]));
    let addr = spawn(
      "127.0.0.1:0",
      metrics.clone()
    )
    .await
    .expect("start isolated metrics listener");
    assert!(addr.ip().is_loopback());
    let response =
      request(addr, "/metrics").await;
    assert_http_response(
      &response,
      "HTTP/1.1 200 OK",
      &metrics.render()
    );

    let not_found =
      request(addr, "/nope").await;
    assert_http_response(
      &not_found,
      "HTTP/1.1 404 Not Found",
      "not found\n"
    );
  }
}
