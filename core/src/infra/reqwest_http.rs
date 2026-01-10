//! Reqwest-backed HTTP client implementing the `Http` port; maps reqwest errors/statuses
//! into domain `HeadResult`/`GetResult` with coarse error kinds.
use crate::domain::model::{ErrorKind, GetResult, HeadResult};
use crate::ports::http::Http;
use chrono::{DateTime, Utc};
use reqwest::{header, StatusCode};
use tracing::{debug, warn};

pub struct ReqwestHttp {
    client: reqwest::Client,
    _user_agent: String,
}

impl ReqwestHttp {
    pub fn new(user_agent: String) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent.clone())
            .pool_idle_timeout(std::time::Duration::from_secs(120))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            _user_agent: user_agent,
        })
    }

    fn classify_error(e: &reqwest::Error) -> ErrorKind {
        if e.is_timeout() {
            ErrorKind::Timeout
        } else if e.is_connect() {
            ErrorKind::ConnectionFailure
        } else {
            ErrorKind::Unexpected
        }
    }

    fn status_error_kind(code: StatusCode) -> Option<ErrorKind> {
        let n = code.as_u16();
        if (400..500).contains(&n) {
            Some(ErrorKind::Http4xx(n))
        } else if (500..600).contains(&n) {
            Some(ErrorKind::Http5xx(n))
        } else {
            None
        }
    }

    fn parse_last_modified(headers: &header::HeaderMap) -> Option<i64> {
        let v = headers.get(header::LAST_MODIFIED)?.to_str().ok()?;
        DateTime::parse_from_rfc2822(v)
            .ok()
            .map(|dt| dt.with_timezone(&Utc).timestamp_millis())
    }

    fn parse_etag(headers: &header::HeaderMap) -> Option<String> {
        headers
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }
}

#[async_trait::async_trait]
impl Http for ReqwestHttp {
    async fn head(&self, url: &str) -> HeadResult {
        let start = tokio::time::Instant::now();
        debug!(url, "HTTP HEAD start");
        match self.client.head(url).send().await {
            Ok(resp) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let status = Some(resp.status().as_u16());
                let etag = Self::parse_etag(resp.headers());
                let last_modified = Self::parse_last_modified(resp.headers());
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
                }
            }
            Err(e) => {
                warn!(url, error = %e, "HTTP HEAD failed");
                let latency_ms = start.elapsed().as_millis() as u64;
                HeadResult {
                    status: None,
                    etag: None,
                    last_modified: None,
                    error: Some(Self::classify_error(&e)),
                    latency_ms,
                }
            }
        }
    }

    async fn get(&self, url: &str) -> GetResult {
        let start = tokio::time::Instant::now();
        debug!(url, "HTTP GET start");
        match self.client.get(url).send().await {
            Ok(resp) => {
                let status = Some(resp.status().as_u16());
                let etag = Self::parse_etag(resp.headers());
                let last_modified = Self::parse_last_modified(resp.headers());
                let body = match resp.bytes().await {
                    Ok(b) => Some(b.to_vec()),
                    Err(e) => {
                        warn!(url, error = %e, "Failed reading body");
                        None
                    }
                };
                let latency_ms = start.elapsed().as_millis() as u64;
                let error = status.and_then(|s| {
                    Self::status_error_kind(
                        StatusCode::from_u16(s).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    )
                });
                GetResult {
                    status,
                    body,
                    etag,
                    last_modified,
                    error,
                    latency_ms,
                }
            }
            Err(e) => {
                warn!(url, error = %e, "HTTP GET failed");
                let latency_ms = start.elapsed().as_millis() as u64;
                GetResult {
                    status: None,
                    body: None,
                    etag: None,
                    last_modified: None,
                    error: Some(Self::classify_error(&e)),
                    latency_ms,
                }
            }
        }
    }
}
