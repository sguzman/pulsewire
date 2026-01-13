use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct FeedSummary {
  pub(crate) id:                String,
  pub(crate) url:               String,
  pub(crate) domain:            String,
  pub(crate) category:          String,
  pub(crate) base_poll_seconds: i64,
  pub(crate) tags: Option<Vec<String>>
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct FeedDetail {
  pub(crate) id:                String,
  pub(crate) url:               String,
  pub(crate) domain:            String,
  pub(crate) category:          String,
  pub(crate) base_poll_seconds: i64,
  pub(crate) tags: Option<Vec<String>>,
  pub(crate) created_at_ms: Option<i64>
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct FeedEntryCounts {
  pub(crate) feed_id: String,
  pub(crate) total_count:          i64,
  pub(crate) unread_count:         i64,
  pub(crate) read_count:           i64,
  pub(crate) last_published_at_ms:
    Option<i64>
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct FolderRow {
  pub(crate) id:   i64,
  pub(crate) name: String
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct FolderFeedRow {
  pub(crate) feed_id: String
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct EntrySummary {
  pub(crate) id:              i64,
  pub(crate) feed_id:         String,
  pub(crate) title: Option<String>,
  pub(crate) link: Option<String>,
  pub(crate) published_at_ms:
    Option<i64>,
  pub(crate) is_read:         bool
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct EntryDetail {
  pub(crate) id:              i64,
  pub(crate) feed_id:         String,
  pub(crate) title: Option<String>,
  pub(crate) link: Option<String>,
  pub(crate) guid: Option<String>,
  pub(crate) published_at_ms:
    Option<i64>,
  pub(crate) category: Option<String>,
  pub(crate) description:
    Option<String>,
  pub(crate) summary: Option<String>,
  pub(crate) is_read:         bool
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct EntryListResponse {
  pub(crate) items: Vec<EntrySummary>,
  pub(crate) next_cursor: Option<i64>,
  pub(crate) next_offset: Option<i64>,
  pub(crate) since:       Option<i64>
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
  pub(crate) token: String
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct SubscriptionRow {
  pub(crate) feed_id: String
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct FavoriteUnreadCount {
  pub(crate) feed_id:      String,
  pub(crate) unread_count: i64
}
