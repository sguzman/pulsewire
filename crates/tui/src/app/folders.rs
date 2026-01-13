use std::collections::HashMap;

use super::App;

impl App {
  pub(crate) fn request_folder_feeds(
    &mut self
  ) {
    let folder_id = self
      .folders
      .get(self.selected_folder)
      .map(|folder| folder.id);

    let Some(folder_id) = folder_id
    else {
      self.folder_feed_ids.clear();
      self.folder_feeds.clear();
      return;
    };

    self.queue_folder_feeds(folder_id);
  }

  pub(crate) fn update_folder_feeds(
    &mut self,
    feed_ids: Vec<String>
  ) {
    self.folder_feed_ids = feed_ids;
    self.folder_feeds_offset = 0;
    self.rebuild_folder_feeds();
  }

  pub(crate) fn rebuild_folder_feeds(
    &mut self
  ) {
    if self.folder_feed_ids.is_empty() {
      self.folder_feeds.clear();
      return;
    }

    let map = self
      .feeds
      .iter()
      .map(|feed| {
        (feed.id.clone(), feed.clone())
      })
      .collect::<HashMap<_, _>>();

    self.folder_feeds = self
      .folder_feed_ids
      .iter()
      .filter_map(|id| {
        map.get(id).cloned()
      })
      .collect();
  }
}
