use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{
  Block,
  Borders,
  Paragraph,
  Wrap
};

use crate::models::{
  EntryDetail,
  EntrySummary,
  FeedDetail,
  FeedSummary,
  FolderRow
};

pub(crate) fn draw_feed_detail(
  frame: &mut Frame,
  area: Rect,
  feed: Option<&FeedSummary>,
  detail: Option<&FeedDetail>,
  title: &str
) {
  let lines = if let Some(feed) = feed {
    let created = detail
      .and_then(|row| row.created_at_ms)
      .map(|ms| ms.to_string())
      .unwrap_or_else(|| {
        "-".to_string()
      });
    vec![
      Line::from(format!(
        "id: {}",
        feed.id
      )),
      Line::from(format!(
        "url: {}",
        feed.url
      )),
      Line::from(format!(
        "domain: {}",
        feed.domain
      )),
      Line::from(format!(
        "category: {}",
        feed.category
      )),
      Line::from(format!(
        "base_poll_seconds: {}",
        feed.base_poll_seconds
      )),
      Line::from(format!(
        "created_at_ms: {}",
        created
      )),
      Line::from(format!(
        "tags: {}",
        feed
          .tags
          .as_ref()
          .map(|tags| tags.join(", "))
          .unwrap_or_else(|| {
            "-".to_string()
          })
      )),
    ]
  } else {
    vec![Line::from("No selection")]
  };

  let widget = Paragraph::new(lines)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title(title)
    )
    .wrap(Wrap {
      trim: true
    });

  frame.render_widget(widget, area);
}

pub(crate) fn draw_entry_detail(
  frame: &mut Frame,
  area: Rect,
  entry: Option<&EntrySummary>,
  detail: Option<&EntryDetail>
) {
  let lines = if let Some(entry) = entry
  {
    let detail_title = detail
      .and_then(|row| {
        row.title.as_deref()
      })
      .or_else(|| {
        entry.title.as_deref()
      })
      .unwrap_or("(untitled)");
    let detail_link = detail
      .and_then(|row| {
        row.link.as_deref()
      })
      .or_else(|| entry.link.as_deref())
      .unwrap_or("-");
    let summary = detail
      .and_then(|row| {
        row.summary.as_deref()
      })
      .unwrap_or("-");
    let description = detail
      .and_then(|row| {
        row.description.as_deref()
      })
      .unwrap_or("-");
    let guid = detail
      .and_then(|row| {
        row.guid.as_deref()
      })
      .unwrap_or("-");

    vec![
      Line::from(format!(
        "id: {}",
        entry.id
      )),
      Line::from(format!(
        "feed: {}",
        entry.feed_id
      )),
      Line::from(format!(
        "read: {}",
        if entry.is_read {
          "yes"
        } else {
          "no"
        }
      )),
      Line::from(format!(
        "published: {:?}",
        entry.published_at_ms
      )),
      Line::from(format!(
        "title: {}",
        detail_title
      )),
      Line::from(format!(
        "link: {}",
        detail_link
      )),
      Line::from(format!(
        "guid: {}",
        guid
      )),
      Line::from(format!(
        "summary: {}",
        summary
      )),
      Line::from(format!(
        "description: {}",
        description
      )),
    ]
  } else {
    vec![Line::from("No selection")]
  };

  let widget = Paragraph::new(lines)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title("Entry Details")
    )
    .wrap(Wrap {
      trim: true
    });

  frame.render_widget(widget, area);
}

pub(crate) fn draw_folder_detail(
  frame: &mut Frame,
  area: Rect,
  folder: Option<&FolderRow>
) {
  let lines =
    if let Some(folder) = folder {
      vec![
        Line::from(format!(
          "id: {}",
          folder.id
        )),
        Line::from(format!(
          "name: {}",
          folder.name
        )),
      ]
    } else {
      vec![Line::from("No selection")]
    };

  let widget = Paragraph::new(lines)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title("Folder Details")
    )
    .wrap(Wrap {
      trim: true
    });

  frame.render_widget(widget, area);
}
