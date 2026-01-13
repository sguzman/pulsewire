use super::{
  App,
  EntriesMode,
  EntriesReadFilter
};

impl App {
  pub(crate) fn entries_filter_label(
    &self
  ) -> &'static str {
    match self.entries_read_filter {
      | EntriesReadFilter::All => "all",
      | EntriesReadFilter::Read => {
        "read"
      }
      | EntriesReadFilter::Unread => {
        "unread"
      }
    }
  }

  pub(crate) fn entries_mode_label(
    &self
  ) -> String {
    match &self.entries_mode {
      | EntriesMode::None => {
        "none".to_string()
      }
      | EntriesMode::Feed(feed_id) => {
        format!("feed:{feed_id}")
      }
      | EntriesMode::Folder(
        folder_id
      ) => {
        format!("folder:{folder_id}")
      }
      | EntriesMode::All => {
        "all entries".to_string()
      }
      | EntriesMode::Search {
        query,
        ..
      } => {
        format!("search:{query}")
      }
    }
  }
}
