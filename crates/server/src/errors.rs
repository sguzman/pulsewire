use axum::{http::StatusCode, response::IntoResponse};

#[derive(Debug)]
pub struct ServerError {
    status: StatusCode,
    message: String,
}

impl ServerError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
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
