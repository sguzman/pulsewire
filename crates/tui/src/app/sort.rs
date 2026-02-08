use std::collections::HashMap;

use super::{
  App,
  SortMode
};
use crate::models::{
  FeedEntryCounts,
  FeedSummary
};

impl App {
  pub(super) fn sort_feed_indices(
    feeds: &[FeedSummary],
    counts: &HashMap<
      String,
      FeedEntryCounts
    >,
    mode: SortMode,
    indices: &mut [usize]
  ) {
    indices.sort_by(|a, b| {
      let left = &feeds[*a];
      let right = &feeds[*b];
      let left_key =
        Self::sort_key_from(
          counts, &left.id, mode
        );
      let right_key =
        Self::sort_key_from(
          counts, &right.id, mode
        );

      right_key
        .0
        .cmp(&left_key.0)
        .then_with(|| {
          right_key.1.cmp(&left_key.1)
        })
        .then_with(|| {
          right.id.cmp(&left.id)
        })
    });
  }

  pub(super) fn sort_favorites(
    &mut self
  ) {
    let mode = self.sort_mode;
    let counts =
      self.feed_counts.clone();
    self.favorites.sort_by(|a, b| {
      let left_key =
        Self::sort_key_from(
          &counts, &a.id, mode
        );
      let right_key =
        Self::sort_key_from(
          &counts, &b.id, mode
        );

      right_key
        .0
        .cmp(&left_key.0)
        .then_with(|| {
          right_key.1.cmp(&left_key.1)
        })
        .then_with(|| b.id.cmp(&a.id))
    });
  }

  pub(super) fn sort_key_from(
    counts: &HashMap<
      String,
      FeedEntryCounts
    >,
    feed_id: &str,
    mode: SortMode
  ) -> (i64, i64) {
    let counts = counts.get(feed_id);
    let unread = counts
      .map(|row| row.unread_count)
      .unwrap_or(0);
    let total = counts
      .map(|row| row.total_count)
      .unwrap_or(0);
    let recent = counts
      .and_then(|row| {
        row.last_published_at_ms
      })
      .unwrap_or(0);

    match mode {
      | SortMode::Unread => {
        (unread, total)
      }
      | SortMode::Total => {
        (total, unread)
      }
      | SortMode::Ratio => {
        let ratio = if total > 0 {
          (unread * 1000) / total
        } else {
          0
        };
        (ratio, total)
      }
      | SortMode::Recent => {
        (recent, total)
      }
    }
  }

  pub(super) fn apply_sort(&mut self) {
    self.rebuild_views();
    self.sort_favorites();
    self.status = format!(
      "Sort: {}",
      self.sort_label()
    );
  }

  pub(super) fn sort_label(
    &self
  ) -> &str {
    match self.sort_mode {
      | SortMode::Unread => "unread",
      | SortMode::Total => "total",
      | SortMode::Ratio => "ratio",
      | SortMode::Recent => "recent"
    }
  }
}
