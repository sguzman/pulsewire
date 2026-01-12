use axum::{http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Debug)]
pub struct ServerError {
    status: StatusCode,
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

impl ServerError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        let code = status_code_to_string(status);
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
            },
        };
        (self.status, axum::Json(body)).into_response()
    }
}

pub fn map_db_error(err: sqlx::Error, message: &str) -> ServerError {
    if is_unique_violation(&err) {
        return ServerError::new(StatusCode::CONFLICT, message);
    }
    ServerError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db_err)
            if db_err.code().as_deref() == Some("23505")
                || db_err.code().as_deref() == Some("2067")
    )
}

fn status_code_to_string(status: StatusCode) -> String {
    match status {
        StatusCode::BAD_REQUEST => "bad_request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::INTERNAL_SERVER_ERROR => "internal_error",
        _ => status.canonical_reason().unwrap_or("error"),
    }
    .to_string()
}
