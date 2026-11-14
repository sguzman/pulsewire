use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

#[derive(Debug)]

pub struct ServerError {
  status:  StatusCode,
  code:    String,
  message: String
}

#[derive(Debug, Serialize)]

struct ErrorEnvelope {
  error: ErrorBody
}

#[derive(Debug, Serialize)]

struct ErrorBody {
  code:    String,
  message: String
}

impl ServerError {
  pub fn new(
    status: StatusCode,
    message: impl Into<String>
  ) -> Self {
    let code =
      status_code_to_string(status);

    Self {
      status,
      code,
      message: message.into()
    }
  }
}

impl IntoResponse for ServerError {
  fn into_response(
    self
  ) -> axum::response::Response {
    // Never send SQL, network or
    // configuration errors back to API
    // clients. Retain stable public
    // error codes.
    let safe_message = if self
      .status
      .is_server_error()
    {
      "internal server error"
        .to_string()
    } else {
      self.message
    };
    let body = ErrorEnvelope {
      error: ErrorBody {
        code:    self.code,
        message: safe_message
      }
    };

    let mut response =
      (self.status, axum::Json(body))
        .into_response();
    if self.status
      == StatusCode::TOO_MANY_REQUESTS
    {
      response.headers_mut().insert(
        axum::http::header::RETRY_AFTER,
        axum::http::HeaderValue::from_static("900")
      );
    }
    response
  }
}

pub fn map_db_error(
  err: sqlx::Error,
  message: &str
) -> ServerError {
  if is_unique_violation(&err) {
    return ServerError::new(
      StatusCode::CONFLICT,
      message
    );
  }

  ServerError::new(
    StatusCode::INTERNAL_SERVER_ERROR,
    err.to_string()
  )
}

pub fn is_unique_violation(
  err: &sqlx::Error
) -> bool {
  matches!(
      err,
      sqlx::Error::Database(db_err)
          if db_err.code().as_deref() == Some("23505")
              || db_err.code().as_deref() == Some("2067")
  )
}

fn status_code_to_string(
  status: StatusCode
) -> String {
  match status {
        StatusCode::BAD_REQUEST => "bad_request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::TOO_MANY_REQUESTS => "rate_limited",
        StatusCode::INTERNAL_SERVER_ERROR => "internal_error",
        _ => status.canonical_reason().unwrap_or("error"),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn internal_errors_never_expose_database_details()
   {
    let internal_details =
      "INTERNAL_DIAGNOSTIC_SENTINEL";
    let response = ServerError::new(
      StatusCode::INTERNAL_SERVER_ERROR,
      internal_details
    )
    .into_response();
    assert_eq!(
      response.status(),
      StatusCode::INTERNAL_SERVER_ERROR
    );
    let body = axum::body::to_bytes(
      response.into_body(),
      4096
    )
    .await
    .expect("error body");
    let parsed: serde_json::Value =
      serde_json::from_slice(&body)
        .expect("valid JSON");
    assert_eq!(
      parsed["error"]["code"],
      "internal_error"
    );
    assert_eq!(
      parsed["error"]["message"],
      "internal server error"
    );
    assert!(
      !String::from_utf8_lossy(&body)
        .contains(internal_details)
    );
  }
}
