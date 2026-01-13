use std::collections::BTreeSet;

use super::App;
use super::util::ensure_offset;
use crate::models::FeedSummary;

impl App {
  pub(super) fn rebuild_views(
    &mut self
  ) {
    let mut categories =
      BTreeSet::new();
    let mut tags = BTreeSet::new();

    for feed in &self.feeds {
      categories
        .insert(feed.category.clone());
      if let Some(feed_tags) =
        &feed.tags
      {
        for tag in feed_tags {
          tags.insert(tag.clone());
        }
      }
    }

    self.categories =
      categories.into_iter().collect();
    self.tags =
      tags.into_iter().collect();

    if let Some(category) =
      &self.filter_category
    {
      if !self
        .categories
        .contains(category)
      {
        self.filter_category = None;
      }
    }

    if let Some(tag) = &self.filter_tag
    {
      if !self.tags.contains(tag) {
        self.filter_tag = None;
      }
    }

    self.feeds_view = self
      .feeds
      .iter()
      .enumerate()
      .filter(|(_, feed)| {
        self.matches_filters(feed)
      })
      .map(|(idx, _)| idx)
      .collect();

    Self::sort_feed_indices(
      &self.feeds,
      &self.feed_counts,
      self.sort_mode,
      &mut self.feeds_view
    );

    self.subscriptions_view = self
      .feeds_view
      .iter()
      .copied()
      .filter(|idx| {
        self.subscriptions.contains(
          &self.feeds[*idx].id
        )
      })
      .collect();

    self.selected_feed =
      self.selected_feed.min(
        self
          .feeds_view
          .len()
          .saturating_sub(1)
      );
    self.selected_subscription =
      self.selected_subscription.min(
        self
          .subscriptions_view
          .len()
          .saturating_sub(1)
      );

    self.feeds_offset = ensure_offset(
      self.selected_feed,
      self.feeds_offset,
      self.feeds_page_size as usize,
      self.feeds_view.len()
    );
    self.subscriptions_offset =
      ensure_offset(
        self.selected_subscription,
        self.subscriptions_offset,
        self.subscriptions_page_size
          as usize,
        self.subscriptions_view.len()
      );

    self.rebuild_folder_feeds();
  }

  pub(super) fn matches_filters(
    &self,
    feed: &FeedSummary
  ) -> bool {
    if let Some(category) =
      &self.filter_category
    {
      if &feed.category != category {
        return false;
      }
    }

    if let Some(tag) = &self.filter_tag
    {
      let matches = feed
        .tags
        .as_ref()
        .map(|tags| {
          tags.iter().any(|t| t == tag)
        })
        .unwrap_or(false);

      if !matches {
        return false;
      }
    }

    if self.hide_empty_feeds
      || self.hide_read_feeds
    {
      let counts =
        self.feed_counts.get(&feed.id);
      let total = counts
        .map(|row| row.total_count)
        .unwrap_or(0);
      let unread = counts
        .map(|row| row.unread_count)
        .unwrap_or(0);

      if self.hide_empty_feeds
        && total == 0
      {
        return false;
      }

      if self.hide_read_feeds
        && unread == 0
      {
        return false;
      }
    }

    true
  }

  pub(super) fn clear_filters(
    &mut self
  ) {
    self.filter_category = None;
    self.filter_tag = None;
    self.selected_feed = 0;
    self.selected_subscription = 0;
    self.feeds_offset = 0;
    self.subscriptions_offset = 0;
    self.rebuild_views();
    self.status = format!(
      "Filters: {}",
      self.filter_summary()
    );
  }

  pub(super) fn filter_summary(
    &self
  ) -> String {
    let category = self
      .filter_category
      .clone()
      .unwrap_or_else(|| {
        "all".to_string()
      });

    let tag = self
      .filter_tag
      .clone()
      .unwrap_or_else(|| {
        "all".to_string()
      });

    format!(
      "category={category} tag={tag}"
    )
  }
}
