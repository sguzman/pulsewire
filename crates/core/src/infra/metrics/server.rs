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
) -> Result<(), String> {
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

        let resp = format!(
          "HTTP/1.1 {status}\r\\
           nContent-Type: text/plain; \
           version=0.0.4\r\\
           nContent-Length: \
           {}\r\nConnection: \
           close\r\n\r\n{}",
          body.len(),
          body
        );

        let _ = stream
          .write_all(resp.as_bytes())
          .await;
      });
    }
  });

  Ok(())
}
