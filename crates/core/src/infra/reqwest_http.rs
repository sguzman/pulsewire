//! Reqwest-backed HTTP client
//! implementing the `Http` port; maps
//! reqwest errors/statuses into domain
//! `HeadResult`/`GetResult` with coarse
//! error kinds.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{
  DateTime,
  Utc
};
use reqwest::{
  StatusCode,
  header
};
use tracing::{
  debug,
  warn
};

use crate::domain::model::{
  ErrorKind,
  GetResult,
  HeadResult
};
use crate::ports::http::Http;
use crate::infra::http_policy::{
  self,
  MAX_REDIRECTS,
  MAX_RESPONSE_BODY_BYTES
};

pub struct ReqwestHttp {
  client:      reqwest::Client,
  _user_agent: String,
  allow_local_test: bool
}

impl ReqwestHttp {
  pub fn new(
    user_agent: String
  ) -> Result<Self, reqwest::Error> {
    Self::new_with_policy(user_agent, false)
  }

  fn new_with_policy(
    user_agent: String,
    allow_local_test: bool
  ) -> Result<Self, reqwest::Error> {
    let client = reqwest::Client::builder()
      .user_agent(user_agent.clone())
      .pool_idle_timeout(Duration::from_secs(120))
      .connect_timeout(Duration::from_secs(10))
      .timeout(Duration::from_secs(30))
      .cookie_store(true)
      .redirect(reqwest::redirect::Policy::custom(
        move |attempt| {
          if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("redirect limit exceeded");
          }
          let Some(previous) = attempt.previous().last() else {
            return attempt.error("redirect history missing");
          };
          if !http_policy::redirect_allowed(
            previous,
            attempt.url(),
            allow_local_test
          ) {
            return attempt.error("unsafe feed redirect");
          }
          attempt.follow()
        }
      ))
      .build()?;

    Ok(Self {
      client,
      _user_agent: user_agent,
      allow_local_test
    })
  }

  fn valid_source(
    &self,
    url: &str
  ) -> bool {
    http_policy::parse_source_url(
      url, self.allow_local_test
    ).is_ok()
  }

  async fn get_bounded(
    &self,
    url: &str,
    cookie_header: Option<&str>,
    extra_headers: Option<&HashMap<String, String>>,
    max_body_bytes: usize
  ) -> GetResult {
    let start = tokio::time::Instant::now();
    if !self.valid_source(url) {
      return GetResult {
        status: None,
        body: None,
        etag: None,
        last_modified: None,
        error: Some(ErrorKind::Unexpected),
        latency_ms: start.elapsed().as_millis() as u64,
        set_cookie_headers: Vec::new()
      };
    }

    let mut req = self.client.get(url);
    if let Some(cookie) = cookie_header
      && !cookie.trim().is_empty()
    {
      req = req.header(header::COOKIE, cookie);
    }
    req = Self::apply_extra_headers(req, extra_headers);

    let limit = max_body_bytes.min(MAX_RESPONSE_BODY_BYTES);
    match req.send().await {
      Ok(mut resp) => {
        let status = Some(resp.status().as_u16());
        let etag = Self::parse_etag(resp.headers());
        let last_modified =
          Self::parse_last_modified(resp.headers());
        let set_cookie_headers =
          Self::parse_set_cookie_headers(resp.headers());
        let mut error = status.and_then(|s| {
          Self::status_error_kind(
            StatusCode::from_u16(s)
              .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
          )
        });
        let mut body = None;
        if resp.content_length().is_some_and(|n| n > limit as u64) {
          warn!(url, limit, "feed body exceeds Content-Length limit");
          error = Some(ErrorKind::Unexpected);
        } else {
          let mut bytes = Vec::new();
          loop {
            match resp.chunk().await {
              Ok(Some(chunk)) => {
                if chunk.len() > limit.saturating_sub(bytes.len()) {
                  warn!(url, limit, "feed body exceeded streaming byte limit");
                  error = Some(ErrorKind::Unexpected);
                  break;
                }
                bytes.extend_from_slice(&chunk);
              }
              Ok(None) => {
                body = Some(bytes);
                break;
              }
              Err(e) => {
                warn!(url, error = %e, "feed body stream failed");
                error = Some(Self::classify_error(&e));
                break;
              }
            }
          }
        }
        GetResult {
          status,
          body,
          etag,
          last_modified,
          error,
          latency_ms: start.elapsed().as_millis() as u64,
          set_cookie_headers
        }
      }
      Err(e) => {
        warn!(url, error = %e, "HTTP GET failed");
        GetResult {
          status: None,
          body: None,
          etag: None,
          last_modified: None,
          error: Some(Self::classify_error(&e)),
          latency_ms: start.elapsed().as_millis() as u64,
          set_cookie_headers: Vec::new()
        }
      }
    }
  }

  fn classify_error(
    e: &reqwest::Error
  ) -> ErrorKind {
    if e.is_timeout() {
      ErrorKind::Timeout
    } else if e.is_connect() {
      ErrorKind::ConnectionFailure
    } else {
      ErrorKind::Unexpected
    }
  }

  fn status_error_kind(
    code: StatusCode
  ) -> Option<ErrorKind> {
    let n = code.as_u16();

    if (400..500).contains(&n) {
      Some(ErrorKind::Http4xx(n))
    } else if (500..600).contains(&n) {
      Some(ErrorKind::Http5xx(n))
    } else {
      None
    }
  }

  fn parse_last_modified(
    headers: &header::HeaderMap
  ) -> Option<i64> {
    let v = headers
      .get(header::LAST_MODIFIED)?
      .to_str()
      .ok()?;

    DateTime::parse_from_rfc2822(v)
      .ok()
      .map(|dt| {
        dt.with_timezone(&Utc)
          .timestamp_millis()
      })
  }

  fn parse_etag(
    headers: &header::HeaderMap
  ) -> Option<String> {
    headers
      .get(header::ETAG)
      .and_then(|v| v.to_str().ok())
      .map(|s| s.to_string())
  }

  fn parse_set_cookie_headers(
    headers: &header::HeaderMap
  ) -> Vec<String> {
    headers
      .get_all(header::SET_COOKIE)
      .iter()
      .filter_map(|v| v.to_str().ok())
      .map(|s| s.to_string())
      .collect()
  }

  fn apply_extra_headers(
    mut req: reqwest::RequestBuilder,
    extra_headers: Option<
      &HashMap<String, String>
    >
  ) -> reqwest::RequestBuilder {
    let Some(extra_headers) =
      extra_headers
    else {
      return req;
    };

    for (name, value) in extra_headers {
      let key = name.trim();

      if key.is_empty()
        || key.starts_with(':')
        || key.eq_ignore_ascii_case(
          "cookie"
        )
      {
        continue;
      }

      let value = value.trim();
      if value.is_empty() {
        continue;
      }

      req = req.header(key, value);
    }

    req
  }
}

#[async_trait::async_trait]
impl Http for ReqwestHttp {
  async fn head(
    &self,
    url: &str,
    cookie_header: Option<&str>,
    extra_headers: Option<
      &HashMap<String, String>
    >
  ) -> HeadResult {
    let start =
      tokio::time::Instant::now();

    debug!(url, "HTTP HEAD start");
    if !self.valid_source(url) {
      return HeadResult {
        status: None,
        etag: None,
        last_modified: None,
        error: Some(ErrorKind::Unexpected),
        latency_ms: start.elapsed().as_millis() as u64,
        set_cookie_headers: Vec::new()
      };
    }

    let mut req = self.client.head(url);

    if let Some(cookie) = cookie_header
      && !cookie.trim().is_empty()
    {
      req = req
        .header(header::COOKIE, cookie);
    }

    req = Self::apply_extra_headers(
      req,
      extra_headers
    );

    match req.send().await {
      | Ok(resp) => {
        let latency_ms =
          start.elapsed().as_millis()
            as u64;

        let status =
          Some(resp.status().as_u16());

        let etag = Self::parse_etag(
          resp.headers()
        );

        let last_modified =
          Self::parse_last_modified(
            resp.headers()
          );

        let set_cookie_headers =
          Self::parse_set_cookie_headers(
            resp.headers()
          );

        let error = status.and_then(|s| {
                    Self::status_error_kind(
                        StatusCode::from_u16(s).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    )
                });

        HeadResult {
          status,
          etag,
          last_modified,
          error,
          latency_ms,
          set_cookie_headers
        }
      }
      | Err(e) => {
        warn!(url, error = %e, "HTTP HEAD failed");

        let latency_ms =
          start.elapsed().as_millis()
            as u64;

        HeadResult {
          status: None,
          etag: None,
          last_modified: None,
          error: Some(
            Self::classify_error(&e)
          ),
          latency_ms,
          set_cookie_headers: Vec::new(
          )
        }
      }
    }
  }

  async fn get(
    &self,
    url: &str,
    cookie_header: Option<&str>,
    extra_headers: Option<&HashMap<String, String>>
  ) -> GetResult {
    self.get_bounded(
      url,
      cookie_header,
      extra_headers,
      MAX_RESPONSE_BODY_BYTES
    )
    .await
  }

  async fn get_with_limit(
    &self,
    url: &str,
    cookie_header: Option<&str>,
    extra_headers: Option<&HashMap<String, String>>,
    max_body_bytes: usize
  ) -> GetResult {
    self.get_bounded(
      url,
      cookie_header,
      extra_headers,
      max_body_bytes
    )
    .await
  }
}
