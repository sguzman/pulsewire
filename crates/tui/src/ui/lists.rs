use std::collections::{
  HashMap,
  HashSet
};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{
  Color,
  Modifier,
  Style
};
use ratatui::widgets::{
  Block,
  Borders,
  List,
  ListItem
};

use super::common::{
  list_state,
  page_bounds
};
use crate::models::{
  EntrySummary,
  FeedEntryCounts,
  FeedSummary,
  FolderRow
};

pub(crate) fn draw_feed_list(
  frame: &mut Frame,
  area: Rect,
  feeds: &[FeedSummary],
  offset: usize,
  page_size: usize,
  selected: usize,
  counts: Option<
    &HashMap<String, FeedEntryCounts>
  >,
  title: &str
) {
  let (start, end) = page_bounds(
    feeds.len(),
    offset,
    page_size
  );

  let items = feeds[start..end]
    .iter()
    .map(|feed| {
      let count = counts
        .and_then(|map| {
          map.get(&feed.id)
        })
        .map(|row| {
          format!(
            "{}/{}/{}",
            row.read_count,
            row.unread_count,
            row.total_count
          )
        })
        .unwrap_or_else(|| {
          "0/0/0".to_string()
        });

      let label = format!(
        "{} [{}] ({})",
        feed.id, feed.domain, count
      );
      ListItem::new(label)
    })
    .collect::<Vec<_>>();

  let list = List::new(items)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title(title)
    )
    .highlight_style(
      Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
    )
    .highlight_symbol("> ");

  let mut state = list_state(
    selected.saturating_sub(start),
    end - start
  );

  frame.render_stateful_widget(
    list, area, &mut state
  );
}

pub(crate) fn draw_feed_view_list(
  frame: &mut Frame,
  area: Rect,
  feeds: &[FeedSummary],
  view: &[usize],
  offset: usize,
  page_size: usize,
  selected: usize,
  subscriptions: Option<
    &HashSet<String>
  >,
  favorites: Option<&HashSet<String>>,
  counts: Option<
    &HashMap<String, FeedEntryCounts>
  >,
  title: &str
) {
  let (start, end) = page_bounds(
    view.len(),
    offset,
    page_size
  );

  let items = view[start..end]
    .iter()
    .map(|idx| {
      let feed = &feeds[*idx];
      let subscribed = subscriptions
        .as_ref()
        .map(|subs| {
          subs.contains(&feed.id)
        })
        .unwrap_or(false);
      let sub_marker = if subscribed {
        "*"
      } else {
        " "
      };

      let favorite = favorites
        .as_ref()
        .map(|favs| {
          favs.contains(&feed.id)
        })
        .unwrap_or(false);
      let fav_marker = if favorite {
        "F"
      } else {
        " "
      };

      let count = counts
        .and_then(|map| {
          map.get(&feed.id)
        })
        .map(|row| {
          format!(
            "{}/{}/{}",
            row.read_count,
            row.unread_count,
            row.total_count
          )
        })
        .unwrap_or_else(|| {
          "0/0/0".to_string()
        });

      let label = format!(
        "{}{} {} [{}] ({})",
        fav_marker,
        sub_marker,
        feed.id,
        feed.domain,
        count
      );
      ListItem::new(label)
    })
    .collect::<Vec<_>>();

  let list = List::new(items)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title(title)
    )
    .highlight_style(
      Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
    )
    .highlight_symbol("> ");

  let mut state = list_state(
    selected.saturating_sub(start),
    end - start
  );

  frame.render_stateful_widget(
    list, area, &mut state
  );
}

pub(crate) fn draw_folder_feed_list(
  frame: &mut Frame,
  area: Rect,
  feeds: &[FeedSummary],
  offset: usize,
  page_size: usize,
  counts: Option<
    &HashMap<String, FeedEntryCounts>
  >,
  title: &str
) {
  let (start, end) = page_bounds(
    feeds.len(),
    offset,
    page_size
  );

  let items = feeds[start..end]
    .iter()
    .map(|feed| {
      let count = counts
        .and_then(|map| {
          map.get(&feed.id)
        })
        .map(|row| {
          format!(
            "{}/{}/{}",
            row.read_count,
            row.unread_count,
            row.total_count
          )
        })
        .unwrap_or_else(|| {
          "0/0/0".to_string()
        });

      let label = format!(
        "{} [{}] ({})",
        feed.id, feed.domain, count
      );
      ListItem::new(label)
    })
    .collect::<Vec<_>>();

  let list = List::new(items).block(
    Block::default()
      .borders(Borders::ALL)
      .title(title)
  );

  frame.render_widget(list, area);
}

pub(crate) fn draw_entries_list(
  frame: &mut Frame,
  area: Rect,
  entries: &[EntrySummary],
  selected: usize
) {
  let items = entries
    .iter()
    .map(|entry| {
      let title = entry
        .title
        .as_deref()
        .unwrap_or("(untitled)");
      let read = if entry.is_read {
        "✓"
      } else {
        "·"
      };
      let label =
        format!("{read} {title}");
      ListItem::new(label)
    })
    .collect::<Vec<_>>();

  let list = List::new(items)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title("Entries")
    )
    .highlight_style(
      Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
    )
    .highlight_symbol("> ");

  let mut state =
    list_state(selected, entries.len());

  frame.render_stateful_widget(
    list, area, &mut state
  );
}

pub(crate) fn draw_folder_list(
  frame: &mut Frame,
  area: Rect,
  folders: &[FolderRow],
  selected: usize,
  offset: usize,
  page_size: usize
) {
  let (start, end) = page_bounds(
    folders.len(),
    offset,
    page_size
  );

  let items = folders[start..end]
    .iter()
    .map(|folder| {
      ListItem::new(format!(
        "{} (#{})",
        folder.name, folder.id
      ))
    })
    .collect::<Vec<_>>();

  let list = List::new(items)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title("Folders")
    )
    .highlight_style(
      Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
    )
    .highlight_symbol("> ");

  let mut state = list_state(
    selected.saturating_sub(start),
    end - start
  );

  frame.render_stateful_widget(
    list, area, &mut state
  );
}
