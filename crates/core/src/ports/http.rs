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
}
