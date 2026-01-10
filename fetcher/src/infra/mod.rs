//! Infrastructure adapters: config loading, logging setup, HTTP client, SQLite repo, time, randomness.
pub mod config;
pub mod database;
pub mod logging;
pub mod metrics;
pub mod postgres_repo;
pub mod random;
pub mod reqwest_http;
pub mod sqlite_repo;
pub mod system_clock;
pub mod time;
