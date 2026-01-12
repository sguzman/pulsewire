use axum::response::IntoResponse;
use axum::http::header::CONTENT_TYPE;

const OPENAPI_JSON: &str = include_str!("../../res/openapi.json");

pub async fn openapi() -> impl IntoResponse {
    ([(CONTENT_TYPE, "application/json")], OPENAPI_JSON)
}
