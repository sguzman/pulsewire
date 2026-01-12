use std::collections::HashSet;

use ratatui::Frame;
use ratatui::layout::{
  Constraint,
  Direction,
  Layout,
  Rect
};
use ratatui::style::{
  Color,
  Modifier,
  Style
};
use ratatui::text::Line;
use ratatui::widgets::{
  Block,
  Borders,
  List,
  ListItem,
  Paragraph,
  Tabs,
  Wrap
};

use crate::app::{
  App,
  LoginField
};
use crate::models::{
  EntrySummary,
  FeedSummary,
  FolderRow
};

pub(crate) fn draw_login(
  frame: &mut Frame,
  app: &App
) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .margin(2)
    .constraints([
      Constraint::Length(3),
      Constraint::Length(3),
      Constraint::Min(3),
      Constraint::Length(3)
    ])
    .split(frame.area());

  let username_style = if matches!(
    app.focus,
    LoginField::Username
  ) {
    Style::default().fg(Color::Yellow)
  } else {
    Style::default()
  };

  let password_style = if matches!(
    app.focus,
    LoginField::Password
  ) {
    Style::default().fg(Color::Yellow)
  } else {
    Style::default()
  };

  let username = Paragraph::new(
    app.username.as_str()
  )
  .block(
    Block::default()
      .borders(Borders::ALL)
      .title("Username")
  )
  .style(username_style);

  let masked =
    "*".repeat(app.password.len());

  let password = Paragraph::new(masked)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title("Password")
    )
    .style(password_style);

  let help =
    Paragraph::new(app.status.as_str())
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title("Status")
      )
      .wrap(Wrap {
        trim: true
      });

  frame
    .render_widget(username, chunks[0]);
  frame
    .render_widget(password, chunks[1]);
  frame.render_widget(help, chunks[2]);
  frame.render_widget(
    Paragraph::new(
      "Enter to login | Tab to switch \
       | q to quit"
    )
    .block(
      Block::default()
        .borders(Borders::ALL)
    ),
    chunks[3]
  );
}

pub(crate) fn draw_main(
  frame: &mut Frame,
  app: &App
) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(3),
      Constraint::Min(3),
      Constraint::Length(3)
    ])
    .split(frame.area());

  let titles = [
    "Feeds (1)",
    "Entries (2)",
    "Favorites (3)",
    "Folders (4)"
  ]
  .iter()
  .map(|t| {
    Line::styled(
      *t,
      Style::default().fg(Color::White)
    )
  })
  .collect::<Vec<_>>();

  let tabs = Tabs::new(titles)
    .select(app.tab)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title("feedrv3")
    )
    .highlight_style(
      Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
    );

  frame.render_widget(tabs, chunks[0]);

  let content = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Percentage(60),
      Constraint::Percentage(40)
    ])
    .split(chunks[1]);

  match app.tab {
    | 0 => {
      draw_feed_list(
        frame,
        content[0],
        &app.feeds,
        Some(&app.subscriptions),
        app.selected_feed,
        "Feeds"
      );
      draw_feed_detail(
        frame,
        content[1],
        app
          .feeds
          .get(app.selected_feed),
        "Feed Details"
      );
    }
    | 1 => {
      draw_entries_list(
        frame,
        content[0],
        &app.entries,
        app.selected_entry
      );
      draw_entry_detail(
        frame,
        content[1],
        app
          .entries
          .get(app.selected_entry)
      );
    }
    | 2 => {
      draw_feed_list(
        frame,
        content[0],
        &app.favorites,
        None,
        app.selected_favorite,
        "Favorites"
      );
      draw_feed_detail(
        frame,
        content[1],
        app
          .favorites
          .get(app.selected_favorite),
        "Favorite Details"
      );
    }
    | _ => {
      draw_folder_list(
        frame,
        content[0],
        &app.folders,
        app.selected_folder
      );
      draw_folder_detail(
        frame,
        content[1],
        app
          .folders
          .get(app.selected_folder)
      );
    }
  }

  let footer =
    Paragraph::new(app.status.as_str())
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title("Status")
      )
      .wrap(Wrap {
        trim: true
      });

  frame
    .render_widget(footer, chunks[2]);
}

fn draw_feed_detail(
  frame: &mut Frame,
  area: Rect,
  feed: Option<&FeedSummary>,
  title: &str
) {
  let lines = if let Some(feed) = feed {
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

fn draw_entry_detail(
  frame: &mut Frame,
  area: Rect,
  entry: Option<&EntrySummary>
) {
  let lines = if let Some(entry) = entry
  {
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
        entry
          .title
          .as_deref()
          .unwrap_or("(untitled)")
      )),
      Line::from(format!(
        "link: {}",
        entry
          .link
          .as_deref()
          .unwrap_or("-")
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

fn draw_folder_detail(
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

fn draw_feed_list(
  frame: &mut Frame,
  area: Rect,
  feeds: &[FeedSummary],
  subscriptions: Option<
    &HashSet<String>
  >,
  selected: usize,
  title: &str
) {
  let items = feeds
    .iter()
    .map(|feed| {
      let subscribed = subscriptions
        .as_ref()
        .map(|subs| {
          subs.contains(&feed.id)
        })
        .unwrap_or(false);
      let marker = if subscribed {
        "*"
      } else {
        " "
      };
      let label = format!(
        "{} {} [{}]",
        marker, feed.id, feed.domain
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

  let mut state =
    list_state(selected, feeds.len());

  frame.render_stateful_widget(
    list, area, &mut state
  );
}

fn draw_entries_list(
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

fn draw_folder_list(
  frame: &mut Frame,
  area: Rect,
  folders: &[FolderRow],
  selected: usize
) {
  let items = folders
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

  let mut state =
    list_state(selected, folders.len());

  frame.render_stateful_widget(
    list, area, &mut state
  );
}

fn list_state(
  selected: usize,
  len: usize
) -> ratatui::widgets::ListState {
  let mut state =
    ratatui::widgets::ListState::default();

  if len > 0 {
    state.select(Some(
      selected
        .min(len.saturating_sub(1))
    ));
  }

  state
}
