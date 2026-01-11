use sqlx::{Pool, Postgres, Sqlite};

#[derive(Clone)]
pub struct AppState {
    pub sqlite: Option<Pool<Sqlite>>,
    pub postgres: Option<Pool<Postgres>>,
    pub fetcher_schema: Option<String>,
    pub token_ttl_seconds: u64,
}
