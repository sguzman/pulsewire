use axum::http::{
  HeaderName,
  header
};
use axum::response::{
  IntoResponse,
  Response
};

const PAGE: &str = include_str!(
  "../../res/reader/index.html"
);
const SCRIPT: &str = include_str!(
  "../../res/reader/app.js"
);
const STYLE: &str = include_str!(
  "../../res/reader/styles.css"
);

fn asset(
  body: &'static str,
  mime: &'static str
) -> Response {
  (
    [
      (header::CONTENT_TYPE, mime),
      (
        header::CACHE_CONTROL,
        "no-store"
      ),
      (
        HeaderName::from_static(
          "x-content-type-options"
        ),
        "nosniff"
      ),
      (
        HeaderName::from_static(
          "content-security-policy"
        ),
        "default-src 'none'; \
         script-src 'self'; style-src \
         'self'; connect-src 'self'; \
         img-src 'self'; base-uri \
         'none'; form-action 'none'; \
         object-src 'none'; \
         frame-ancestors 'none'"
      )
    ],
    body
  )
    .into_response()
}

pub async fn page() -> Response {
  asset(
    PAGE,
    "text/html; charset=utf-8"
  )
}

pub async fn script() -> Response {
  asset(
    SCRIPT,
    "text/javascript; charset=utf-8"
  )
}

pub async fn style() -> Response {
  asset(
    STYLE,
    "text/css; charset=utf-8"
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn reader_assets_are_embedded_with_safe_headers()
   {
    for response in [
      page().await,
      script().await,
      style().await
    ] {
      assert_eq!(
        response.status(),
        axum::http::StatusCode::OK
      );
      assert_eq!(
        response
          .headers()
          .get("x-content-type-options")
          .unwrap(),
        "nosniff"
      );
      assert!(
        response
          .headers()
          .get(
            "content-security-policy"
          )
          .unwrap()
          .to_str()
          .unwrap()
          .contains(
            "script-src 'self'"
          )
      );
      assert_eq!(
        response
          .headers()
          .get(header::CACHE_CONTROL)
          .unwrap(),
        "no-store"
      );
      let body = axum::body::to_bytes(
        response.into_body(),
        256 * 1024
      )
      .await
      .expect("reader asset body");
      assert!(!body.is_empty());
    }
  }
}
