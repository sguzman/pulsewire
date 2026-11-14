use serde::{
  Deserialize,
  Serialize
};

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FeedSummary {
  pub id:                String,
  pub url:               String,
  pub domain:            String,
  pub category:          String,
  pub base_poll_seconds: i64,
  pub tags: Option<Vec<String>>
}

#[derive(Deserialize)]

pub struct CreateUserRequest {
  pub username: String,
  pub password: String
}

impl std::fmt::Debug
  for CreateUserRequest
{
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    f.debug_struct("CreateUserRequest")
      .field("username", &self.username)
      .field("password", &"<redacted>")
      .finish()
  }
}

#[derive(Deserialize)]

pub struct LoginRequest {
  pub username: String,
  pub password: String
}

impl std::fmt::Debug for LoginRequest {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    f.debug_struct("LoginRequest")
      .field("username", &self.username)
      .field("password", &"<redacted>")
      .finish()
  }
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct EntrySummary {
  pub id:              i64,
  pub feed_id:         String,
  pub title:           Option<String>,
  pub link:            Option<String>,
  pub published_at_ms: Option<i64>,
  pub is_read:         bool
}

#[derive(Debug, Deserialize)]

pub struct EntryListQuery {
  pub read:    Option<String>,
  pub limit:   Option<u32>,
  pub offset:  Option<u32>,
  pub feed_id: Option<String>,
  pub since:   Option<i64>
}

#[derive(Debug, Deserialize)]

pub struct SearchQuery {
  pub q:       String,
  pub limit:   Option<u32>,
  pub offset:  Option<u32>,
  pub feed_id: Option<String>,
  pub read:    Option<String>
}

#[derive(Debug, Serialize)]

pub struct EntryListResponse {
  pub items:       Vec<EntrySummary>,
  pub next_cursor: Option<i64>,
  pub next_offset: Option<i64>,
  pub since:       Option<i64>
}

#[derive(Debug, Deserialize)]

pub struct EntryBatchRequest {
  pub item_ids: Vec<i64>
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FeedDetail {
  pub id:                String,
  pub url:               String,
  pub domain:            String,
  pub category:          String,
  pub base_poll_seconds: i64,
  pub tags: Option<Vec<String>>,
  pub created_at_ms:     Option<i64>
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FavoriteUnreadCount {
  pub feed_id:      String,
  pub unread_count: i64
}

#[derive(Debug, Deserialize)]

pub struct FolderEntriesQuery {
  pub read:   Option<String>,
  pub limit:  Option<u32>,
  pub offset: Option<u32>,
  pub since:  Option<i64>
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct EntryDetail {
  pub id:              i64,
  pub feed_id:         String,
  pub title:           Option<String>,
  pub link:            Option<String>,
  pub guid:            Option<String>,
  pub published_at_ms: Option<i64>,
  pub category:        Option<String>,
  pub description:     Option<String>,
  pub summary:         Option<String>,
  pub is_read:         bool
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FeedUnreadCount {
  pub feed_id:      String,
  pub unread_count: i64
}

#[derive(Debug, Serialize)]

pub struct FeedEntryCounts {
  pub feed_id:              String,
  pub total_count:          i64,
  pub unread_count:         i64,
  pub read_count:           i64,
  pub last_published_at_ms: Option<i64>
}

#[derive(Debug, Serialize)]

pub struct UnreadCountResponse {
  pub count: i64
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FolderRow {
  pub id:   i64,
  pub name: String
}

#[derive(Debug, Deserialize)]

pub struct FolderCreateRequest {
  pub name: String
}

#[derive(Debug, Deserialize)]

pub struct FolderUpdateRequest {
  pub name: String
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FolderFeedRow {
  pub feed_id: String
}

#[derive(Debug, Deserialize)]

pub struct FolderFeedRequest {
  pub feed_id: String
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FolderFeedUnreadCount {
  pub feed_id:      String,
  pub unread_count: i64
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct FolderUnreadCount {
  pub folder_id:    i64,
  pub unread_count: i64
}

#[derive(Debug, Serialize)]

pub struct UserResponse {
  pub id:       i64,
  pub username: String
}

#[derive(Debug, Deserialize)]

pub struct FavoriteRequest {
  pub feed_id: String
}

#[derive(Debug, Deserialize)]

pub struct FavoriteListQuery {
  pub limit:  Option<u32>,
  pub offset: Option<u32>
}

#[derive(Deserialize)]

pub struct PasswordChangeRequest {
  pub current_password: String,
  pub new_password:     String
}

impl std::fmt::Debug
  for PasswordChangeRequest
{
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    f.debug_struct(
      "PasswordChangeRequest"
    )
    .field(
      "current_password",
      &"<redacted>"
    )
    .field(
      "new_password",
      &"<redacted>"
    )
    .finish()
  }
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct TokenSummary {
  pub id:         i64,
  pub expires_at: String,
  pub created_at: String
}

#[derive(Serialize)]

pub struct TokenResponse {
  pub token:      String,
  pub token_type: String,
  pub expires_in: u64
}

impl std::fmt::Debug for TokenResponse {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    f.debug_struct("TokenResponse")
      .field("token", &"<redacted>")
      .field(
        "token_type",
        &self.token_type
      )
      .field(
        "expires_in",
        &self.expires_in
      )
      .finish()
  }
}

#[derive(Debug, Deserialize)]

pub struct SubscriptionRequest {
  pub feed_id: String
}

#[derive(
  Debug, Serialize, sqlx::FromRow,
)]

pub struct SubscriptionRow {
  pub feed_id: String
}

#[cfg(test)]
mod auth_debug_redaction_tests {
  use super::*;

  #[test]
  fn never_debug_print_passwords_or_bearer_tokens()
   {
    let secret =
      "pw023-secret-debug-sentinel";
    let create = CreateUserRequest {
      username: "reader".to_string(),
      password: secret.to_string()
    };
    let login = LoginRequest {
      username: "reader".to_string(),
      password: secret.to_string()
    };
    let change =
      PasswordChangeRequest {
        current_password: secret
          .to_string(),
        new_password:     secret
          .to_string()
      };
    let token = TokenResponse {
      token:      secret.to_string(),
      token_type: "bearer".to_string(),
      expires_in: 3600
    };
    for printed in [
      format!("{create:?}"),
      format!("{login:?}"),
      format!("{change:?}"),
      format!("{token:?}")
    ] {
      assert!(
        printed.contains("<redacted>")
      );
      assert!(
        !printed.contains(secret)
      );
    }
    assert!(
      format!("{login:?}")
        .contains("reader")
    );
    assert!(
      format!("{token:?}")
        .contains("bearer")
    );
  }
}
