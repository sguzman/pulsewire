use super::util::ensure_offset;
use super::{
  App,
  EntriesMode
};

impl App {
  pub(super) fn next_list_page(
    &mut self
  ) {
    let len = self.list_len_for_tab();
    if len == 0 {
      return;
    }

    let page = self.page_size_for_tab();
    let offset =
      self.list_offset_value();
    if offset + page >= len {
      return;
    }

    let next = (offset + page)
      .min(len.saturating_sub(1));
    self.set_list_offset(next);
    self
      .update_selected_for_offset(next);
    if let Some((current, total)) =
      self.list_page_info()
    {
      self.status = format!(
        "Page {current}/{total} \
         (offset {next})"
      );
    } else {
      self.status =
        format!("Page offset {}", next);
    }
  }

  pub(super) fn prev_list_page(
    &mut self
  ) {
    let len = self.list_len_for_tab();
    if len == 0 {
      return;
    }

    let page = self.page_size_for_tab();
    let offset =
      self.list_offset_value();
    if offset == 0 {
      return;
    }

    let next =
      offset.saturating_sub(page);
    self.set_list_offset(next);
    self
      .update_selected_for_offset(next);
    if let Some((current, total)) =
      self.list_page_info()
    {
      self.status = format!(
        "Page {current}/{total} \
         (offset {next})"
      );
    } else {
      self.status =
        format!("Page offset {}", next);
    }
  }

  pub(super) fn update_selected_for_offset(
    &mut self,
    offset: usize
  ) {
    let page = self.page_size_for_tab();
    let selected =
      self.selected_for_tab();
    if *selected < offset {
      *selected = offset;
    } else if *selected >= offset + page
    {
      *selected = offset;
    }
  }

  pub(super) fn list_page_info(
    &self
  ) -> Option<(usize, usize)> {
    let len = self.list_len_for_tab();
    if len == 0 {
      return None;
    }

    let page = self.page_size_for_tab();
    let total_pages =
      (len + page - 1) / page;
    let current_page =
      self.list_offset_value() / page
        + 1;

    Some((current_page, total_pages))
  }

  pub(super) fn entries_page_info(
    &self
  ) -> Option<(usize, usize)> {
    let feed_id =
      match &self.entries_mode {
        | EntriesMode::Feed(
          feed_id
        ) => feed_id,
        | _ => return None
      };

    let total = self
      .feed_counts
      .get(feed_id)
      .map(|row| row.total_count)
      .unwrap_or(0);

    if total <= 0 {
      return None;
    }

    let page_size =
      self.entries_page_size as i64;
    let total_pages =
      ((total + page_size - 1)
        / page_size) as usize;
    let current_page =
      (self.entries_offset / page_size)
        as usize
        + 1;

    Some((current_page, total_pages))
  }

  pub(super) fn list_len_for_tab(
    &self
  ) -> usize {
    match self.tab {
      | 0 => self.feeds_view.len(),
      | 2 => self.favorites.len(),
      | 3 => self.folders.len(),
      | 4 => {
        self.subscriptions_view.len()
      }
      | _ => 0
    }
  }

  pub(super) fn page_size_for_tab(
    &self
  ) -> usize {
    match self.tab {
      | 0 => {
        self.feeds_page_size as usize
      }
      | 2 => {
        self.favorites_page_size
          as usize
      }
      | 3 => {
        self.folders_page_size as usize
      }
      | 4 => {
        self.subscriptions_page_size
          as usize
      }
      | _ => {
        self.feeds_page_size as usize
      }
    }
  }

  pub(super) fn list_offset_value(
    &self
  ) -> usize {
    match self.tab {
      | 0 => self.feeds_offset,
      | 2 => self.favorites_offset,
      | 3 => self.folders_offset,
      | 4 => self.subscriptions_offset,
      | _ => 0
    }
  }

  pub(super) fn set_list_offset(
    &mut self,
    value: usize
  ) {
    match self.tab {
      | 0 => self.feeds_offset = value,
      | 2 => {
        self.favorites_offset = value
      }
      | 3 => {
        self.folders_offset = value
      }
      | 4 => {
        self.subscriptions_offset =
          value
      }
      | _ => {}
    }
  }

  pub(super) fn list_offset_for_tab(
    &mut self
  ) -> &mut usize {
    match self.tab {
      | 0 => &mut self.feeds_offset,
      | 2 => &mut self.favorites_offset,
      | 3 => &mut self.folders_offset,
      | 4 => {
        &mut self.subscriptions_offset
      }
      | _ => &mut self.feeds_offset
    }
  }

  pub(super) fn selected_value_for_tab(
    &self
  ) -> usize {
    match self.tab {
      | 0 => self.selected_feed,
      | 2 => self.selected_favorite,
      | 3 => self.selected_folder,
      | 4 => self.selected_subscription,
      | _ => self.selected_feed
    }
  }

  pub(super) fn selected_for_tab(
    &mut self
  ) -> &mut usize {
    match self.tab {
      | 0 => &mut self.selected_feed,
      | 2 => {
        &mut self.selected_favorite
      }
      | 3 => &mut self.selected_folder,
      | 4 => {
        &mut self.selected_subscription
      }
      | _ => &mut self.selected_feed
    }
  }

  pub(super) fn ensure_visible_for_tab(
    &mut self
  ) {
    let len = self.list_len_for_tab();
    let selected =
      self.selected_value_for_tab();
    let page = self.page_size_for_tab();
    let offset =
      self.list_offset_for_tab();
    *offset = ensure_offset(
      selected, *offset, page, len
    );
  }
}
