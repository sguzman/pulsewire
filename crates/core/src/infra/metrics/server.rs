//! Minimal, bounded metrics HTTP listener.
//! A non-loopback metrics bind MUST be protected by an explicit,
//! private bearer-token file. No user-provided HTTP request data
//! or token bytes are logged.

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::{
  AsyncReadExt,
  AsyncWriteExt
};
use tokio::net::{
  TcpListener,
  TcpStream
};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use super::Metrics;

const MAX_HEADERS: usize = 8192;
const MAX_CONCURRENT: usize = 64;
const MAX_TOKEN_BYTES: usize = 512;
const MIN_TOKEN_BYTES: usize = 32;
const HEADER_TIMEOUT: Duration =
  Duration::from_secs(3);

/// A token file is mandatory when
/// metrics accept non-loopback peers.
/// No insecure fallback is allowed.
fn load_token(
  addr: SocketAddr
) -> Result<Option<Arc<str>>, String>
{
  let file = env::var(
    "PULSEWIRE_METRICS_TOKEN_FILE"
  )
  .ok();

  token_from_file(
    addr,
    file.as_deref()
  )
}

fn token_from_file(
  addr: SocketAddr,
  path: Option<&str>
) -> Result<Option<Arc<str>>, String>
{
  let Some(path) = path
    .filter(|p| !p.is_empty())
  else {
    if !addr.ip().is_loopback() {
      return Err(
        "non-loopback metrics listener \
         requires PULSEWIRE_METRICS_TOKEN_FILE"
          .into()
      );
    }
    return Ok(None);
  };

  let mut value =
    std::fs::read_to_string(path)
      .map_err(|_| {
        "metrics token file unavailable"
          .to_string()
      })?;

  if value.ends_with('\n') {
    value.pop();
    if value.ends_with('\r') {
      value.pop();
    }
  }

  if !(MIN_TOKEN_BYTES..=MAX_TOKEN_BYTES)
    .contains(&value.len())
    || !value
      .bytes()
      .all(|byte| {
        (33..=126).contains(&byte)
      })
  {
    return Err(
      "metrics token file has invalid \
       content"
        .into()
    );
  }

  Ok(Some(Arc::from(value)))
}

fn valid_bearer(
  headers: &str,
  expected: &str
) -> bool {
  let mut seen = 0usize;
  let mut valid = false;

  for line in headers
    .split("\r\n")
    .skip(1)
  {
    let Some((name, value)) =
      line.split_once(':')
    else {
      continue;
    };
    if !name.eq_ignore_ascii_case(
      "authorization"
    ) {
      continue;
    }

    seen += 1;
    let Some(provided) = value
      .trim()
      .strip_prefix("Bearer ")
    else {
      continue;
    };

    let wanted =
      Sha256::digest(expected.as_bytes());
    let found =
      Sha256::digest(provided.as_bytes());
    let mut difference = 0u8;
    for (a, b) in wanted
      .iter()
      .zip(found.iter())
    {
      difference |= a ^ b;
    }
    valid = difference == 0;
  }

  seen == 1 && valid
}

async fn read_request(
  stream: &mut TcpStream
) -> Option<String> {
  timeout(HEADER_TIMEOUT, async {
    let mut buffer = [0u8; MAX_HEADERS];
    let mut used = 0usize;
    loop {
      if used == buffer.len() {
        return None;
      }
      let bytes = stream
        .read(&mut buffer[used..])
        .await
        .ok()?;
      if bytes == 0 {
        return None;
      }
      used += bytes;
      if buffer[..used]
        .windows(4)
        .any(|w| w == b"\r\n\r\n")
      {
        return String::from_utf8(
          buffer[..used].to_vec()
        )
        .ok();
      }
    }
  })
  .await
  .ok()
  .flatten()
}

async fn serve(
  mut stream: TcpStream,
  metrics: Arc<Metrics>,
  token: Option<Arc<str>>
) {
  let Some(request) =
    read_request(&mut stream).await
  else {
    return;
  };

  let first = request
    .split("\r\n")
    .next()
    .unwrap_or("");
  let method = first
    .split_whitespace()
    .next();
  let path = first
    .split_whitespace()
    .nth(1);
  let version = first
    .split_whitespace()
    .nth(2);

  let (status, body) = if method
    != Some("GET")
    || version != Some("HTTP/1.1")
  {
    (
      "405 Method Not Allowed",
      "not allowed\n".to_string()
    )
  } else if path != Some("/metrics")
  {
    (
      "404 Not Found",
      "not found\n".to_string()
    )
  } else if token.as_ref().is_some_and(
    |t| !valid_bearer(&request, t)
  ) {
    (
      "401 Unauthorized",
      "unauthorized\n".to_string()
    )
  } else {
    ("200 OK", metrics.render())
  };

  let mut response = format!(
    "HTTP/1.1 {status}\r\n"
  );
  response.push_str(
    "Content-Type: text/plain; \
     version=0.0.4\r\n"
  );
  response.push_str(
    "Cache-Control: no-store\r\n"
  );
  response.push_str(&format!(
    "Content-Length: {}\r\n",
    body.len()
  ));
  if status == "401 Unauthorized" {
    response.push_str(
      "WWW-Authenticate: Bearer\r\n"
    );
  }
  response.push_str(
    "Connection: close\r\n\r\n"
  );
  response.push_str(&body);
  let _ = stream
    .write_all(response.as_bytes())
    .await;
}

pub(super) async fn spawn(
  bind: &str,
  metrics: Arc<Metrics>
) -> Result<SocketAddr, String> {
  let addr: SocketAddr =
    bind.parse().map_err(|_| {
      "invalid metrics bind".to_string()
    })?;
  let token = load_token(addr)?;

  spawn_with_token(addr, metrics, token)
    .await
}

async fn spawn_with_token(
  addr: SocketAddr,
  metrics: Arc<Metrics>,
  token: Option<Arc<str>>
) -> Result<SocketAddr, String> {
  let listener =
    TcpListener::bind(addr)
      .await
      .map_err(|_| {
        "failed to bind metrics listener"
          .to_string()
      })?;
  let bound = listener
    .local_addr()
    .map_err(|_| {
      "failed to inspect metrics listener"
        .to_string()
    })?;

  let permits =
    Arc::new(Semaphore::new(
      MAX_CONCURRENT
    ));

  tokio::spawn(async move {
    loop {
      let (stream, _) =
        match listener.accept().await {
          | Ok(pair) => pair,
          | Err(_) => continue
        };
      let Ok(permit) = permits
        .clone()
        .try_acquire_owned()
      else {
        continue;
      };
      let m = metrics.clone();
      let t = token.clone();
      tokio::spawn(async move {
        let _permit = permit;
        serve(stream, m, t).await;
      });
    }
  });

  Ok(bound)
}

#[cfg(test)]
mod metrics_http_tests {
  use std::net::{
    IpAddr,
    Ipv4Addr
  };

  use super::*;

  fn address(
    ip: [u8; 4]
  ) -> SocketAddr {
    SocketAddr::new(
      IpAddr::V4(Ipv4Addr::from(ip)),
      9898
    )
  }

  fn fixture_path(
    name: &str
  ) -> std::path::PathBuf {
    env::temp_dir().join(format!(
      "pulsewire-metrics-token-{}-{name}",
      std::process::id()
    ))
  }

  async fn request(
    addr: SocketAddr,
    raw: &str
  ) -> String {
    let mut stream =
      TcpStream::connect(addr)
        .await
        .expect("connect metrics listener");
    stream
      .write_all(raw.as_bytes())
      .await
      .expect("send metrics request");
    let mut bytes = Vec::new();
    stream
      .read_to_end(&mut bytes)
      .await
      .expect("receive metrics response");
    String::from_utf8(bytes)
      .expect("UTF-8 metrics response")
  }

  fn assert_response(
    response: &str,
    status: &str,
    expected: &str
  ) {
    let (headers, body) = response
      .split_once("\r\n\r\n")
      .expect("valid HTTP response");
    assert_eq!(
      headers.split("\r\n").next(),
      Some(status)
    );
    assert!(headers.contains(
      "\r\nContent-Type: text/plain; \
       version=0.0.4"
    ));
    let len = headers
      .split("\r\n")
      .find_map(|s| {
        s.strip_prefix(
          "Content-Length: "
        )
      })
      .expect("content length")
      .parse::<usize>()
      .expect("numeric length");
    assert_eq!(len, body.len());
    assert_eq!(body, expected);
  }

  #[test]
  fn non_loopback_requires_token_file()
   {
    assert!(
      token_from_file(
        address([0, 0, 0, 0]),
        None
      )
      .is_err()
    );
    assert!(
      token_from_file(
        address([127, 0, 0, 1]),
        None
      )
      .unwrap()
      .is_none()
    );
  }

  #[test]
  fn token_file_is_private_and_validated()
   {
    let path = fixture_path("valid");
    let value = "m".repeat(48);
    std::fs::write(
      &path,
      format!("{value}\r\n")
    )
    .expect("write fixture");
    assert_eq!(
      token_from_file(
        address([0, 0, 0, 0]),
        path.to_str()
      )
      .unwrap()
      .as_deref(),
      Some(value.as_str())
    );
    std::fs::write(
      &path,
      format!("{value}\nsecond")
    )
    .expect("malformed fixture");
    assert!(
      token_from_file(
        address([0, 0, 0, 0]),
        path.to_str()
      )
      .unwrap_err()
      .contains("invalid")
    );
    std::fs::remove_file(path)
      .expect("remove fixture");
    let error = token_from_file(
      address([0, 0, 0, 0]),
      Some("/missing/private/token")
    )
    .unwrap_err();
    assert!(!error.contains("private"));
  }

  #[tokio::test]
  async fn metrics_http_uses_valid_crlf_and_content_length()
   {
    let metrics =
      Arc::new(Metrics::new(&[]));
    let addr = spawn_with_token(
      address([127, 0, 0, 1])
        .with_port(0),
      metrics.clone(),
      None
    )
    .await
    .expect("start isolated listener");
    let response = request(
      addr,
      "GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n"
    )
    .await;
    assert_response(
      &response,
      "HTTP/1.1 200 OK",
      &metrics.render()
    );
    let other = request(
      addr,
      "GET /nope HTTP/1.1\r\nHost: localhost\r\n\r\n"
    )
    .await;
    assert_response(
      &other,
      "HTTP/1.1 404 Not Found",
      "not found\n"
    );
  }

  #[tokio::test]
  async fn metrics_bearer_protection_rejects_spoofing()
   {
    let metrics =
      Arc::new(Metrics::new(&[]));
    let secret =
      "fixture-secret-credential-for-metrics-123";
    let addr = spawn_with_token(
      SocketAddr::new(
        IpAddr::V4(
          Ipv4Addr::LOCALHOST
        ),
        0
      ),
      metrics.clone(),
      Some(Arc::from(secret))
    )
    .await
    .expect("protected test listener");

    for auth in [
      "",
      "Authorization: Bearer wrong\r\n",
      "Authorization: Basic invalid\r\n",
      "Authorization: Bearer fixture-secret-credential-for-metrics-123\r\nAuthorization: Bearer wrong\r\n"
    ] {
      let request_text = format!(
        "GET /metrics HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n"
      );
      assert_response(
        &request(addr, &request_text)
          .await,
        "HTTP/1.1 401 Unauthorized",
        "unauthorized\n"
      );
    }

    let valid = format!(
      "GET /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {secret}\r\n\r\n"
    );
    assert_response(
      &request(addr, &valid).await,
      "HTTP/1.1 200 OK",
      &metrics.render()
    );
  }
}
