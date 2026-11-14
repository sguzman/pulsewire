//! HTTP abstraction returning
//! lightweight HEAD/GET results.

use std::collections::HashMap;

use crate::domain::model::{
  GetResult,
  HeadResult
};

#[async_trait::async_trait]
pub trait Http: Send + Sync {
  async fn head(
    &self,
    url: &str,
    cookie_header: Option<&str>,
    extra_headers: Option<
      &HashMap<String, String>
    >
  ) -> HeadResult;

  async fn get(
    &self,
    url: &str,
    cookie_header: Option<&str>,
    extra_headers: Option<
      &HashMap<String, String>
    >
  ) -> GetResult;

  /// The concrete HTTP adapter enforces
  /// this bound while streaming; the
  /// scheduler also checks returned bytes
  /// so test/mocked adapters cannot skip it.
  async fn get_with_limit(
    &self,
    url: &str,
    cookie_header: Option<&str>,
    extra_headers: Option<&HashMap<String, String>>,
    _max_body_bytes: usize
  ) -> GetResult {
    self.get(url, cookie_header, extra_headers).await
  }
}
