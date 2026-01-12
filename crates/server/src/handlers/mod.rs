mod auth;
mod docs;
mod entries;
mod favorites;
mod feeds;
mod folders;
mod health;
mod subscriptions;
mod users;

use axum::Router;
use axum::routing::{
  delete,
  get,
  patch,
  post
};

use crate::app_state::AppState;

pub fn router(
  state: AppState
) -> Router {
  Router::new()
        .route("/health", get(health::health))
        .route("/openapi.json", get(docs::openapi))
        .route("/docs", get(docs::openapi_html))
        .route("/v1/feeds", get(feeds::list_feeds))
        .route("/v1/feeds/:feed_id", get(feeds::feed_detail))
        .route("/v1/favorites", get(favorites::list_favorites))
        .route("/v1/favorites/unread/count", get(favorites::favorites_unread_count))
        .route("/v1/favorites/unread/counts", get(favorites::favorites_unread_counts))
        .route("/v1/favorites", post(favorites::create_favorite))
        .route("/v1/favorites/:feed_id", delete(favorites::delete_favorite))
        .route("/v1/folders", get(folders::list_folders))
        .route("/v1/folders", post(folders::create_folder))
        .route("/v1/folders/:folder_id", patch(folders::update_folder))
        .route("/v1/folders/:folder_id", delete(folders::delete_folder))
        .route("/v1/folders/:folder_id/feeds", get(folders::list_folder_feeds))
        .route("/v1/folders/:folder_id/entries", get(folders::list_folder_entries))
        .route("/v1/folders/:folder_id/feeds", post(folders::add_folder_feed))
        .route("/v1/folders/:folder_id/feeds/:feed_id", delete(folders::delete_folder_feed))
        .route("/v1/folders/unread/counts", get(folders::folder_unread_counts))
        .route("/v1/folders/:folder_id/unread/counts", get(folders::folder_feed_unread_counts))
        .route("/v1/users", post(users::create_user))
        .route("/v1/users/me", delete(users::delete_user))
        .route("/v1/users/password-reset/request", post(users::request_password_reset))
        .route("/v1/users/password-reset/confirm", post(users::confirm_password_reset))
        .route("/v1/users/password", post(users::change_password))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/rotate", post(auth::rotate_token))
        .route("/v1/auth/tokens", get(auth::list_tokens))
        .route("/v1/auth/tokens/:token_id", delete(auth::revoke_token))
        .route("/v1/entries", get(entries::list_entries))
        .route("/v1/entries/search", get(entries::search_entries))
        .route("/v1/entries/:item_id", get(entries::entry_detail))
        .route("/v1/entries/unread/count", get(entries::unread_count))
        .route("/v1/feeds/unread/counts", get(entries::feed_unread_counts))
        .route("/v1/feeds/counts", get(entries::feed_entry_counts))
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
