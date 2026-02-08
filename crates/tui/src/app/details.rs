use super::App;

impl App {
  pub(super) fn prefetch_selection_details(
    &mut self
  ) {
    match self.tab {
      | 0 | 2 | 4 => {
        if let Some(feed) =
          self.current_feed_context()
          && !self
            .feed_details
            .contains_key(&feed.id)
        {
          self
            .queue_feed_detail(feed.id);
        }
      }
      | 1 => {
        if let Some(entry) = self
          .entries
          .get(self.selected_entry)
          && !self
            .entry_details
            .contains_key(&entry.id)
        {
          self.queue_entry_detail(
            entry.id
          );
        }
      }
      | _ => {}
    }
  }
}
