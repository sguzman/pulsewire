use axum::response::IntoResponse;
use axum::http::header::CONTENT_TYPE;

const OPENAPI_JSON: &str = include_str!("../../res/openapi.json");
const OPENAPI_HTML: &str = include_str!("../../res/openapi.html");

pub async fn openapi() -> impl IntoResponse {
    ([(CONTENT_TYPE, "application/json")], OPENAPI_JSON)
}


pub async fn openapi_html() -> impl IntoResponse {
    ([(CONTENT_TYPE, "text/html; charset=utf-8")], OPENAPI_HTML)
}
