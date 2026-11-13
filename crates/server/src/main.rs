mod app_state;
mod auth;
mod config;
mod db;
mod errors;
mod handlers;
mod logging;
mod login_throttle;
mod models;
mod schema;
mod session_issue;
mod startup;

use crate::config::ConfigError;

#[tokio::main]
async fn main()
-> Result<(), ConfigError> {
  startup::run().await
}
