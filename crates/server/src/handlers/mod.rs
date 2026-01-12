mod auth;
mod entries;
mod feeds;
mod health;
mod subscriptions;
mod users;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::app_state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/feeds", get(feeds::list_feeds))
        .route("/v1/users", post(users::create_user))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/entries", get(entries::list_entries))
        .route("/v1/entries/:item_id", get(entries::entry_detail))
        .route("/v1/entries/unread/count", get(entries::unread_count))
        .route("/v1/feeds/unread/counts", get(entries::feed_unread_counts))
        .route("/v1/feeds/:feed_id/entries", get(entries::list_feed_entries))
        .route("/v1/entries/read", post(entries::mark_entries_read))
        .route("/v1/entries/read", delete(entries::mark_entries_unread))
        .route("/v1/entries/:item_id/read", get(entries::read_state))
        .route("/v1/entries/:item_id/read", post(entries::mark_read))
        .route("/v1/entries/:item_id/read", delete(entries::mark_unread))
        .route("/v1/subscriptions", get(subscriptions::list_subscriptions))
        .route("/v1/subscriptions", post(subscriptions::create_subscription))
        .route("/v1/subscriptions/:feed_id", delete(subscriptions::delete_subscription))
        .with_state(state)
}
