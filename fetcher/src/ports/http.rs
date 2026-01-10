//! HTTP abstraction returning lightweight HEAD/GET results.
use crate::domain::model::{GetResult, HeadResult};

#[async_trait::async_trait]
pub trait Http: Send + Sync {
    async fn head(&self, url: &str) -> HeadResult;
    async fn get(&self, url: &str) -> GetResult;
}
