use ratatui::Frame;
use ratatui::layout::{
  Constraint,
  Direction,
  Layout
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
  Paragraph,
  Tabs,
  Wrap
};

use super::detail::{
  draw_entry_detail,
  draw_feed_detail
};
use super::lists::{
  draw_entries_list,
  draw_feed_list,
  draw_feed_view_list,
  draw_folder_feed_list,
  draw_folder_list
};
use super::modal::{
  draw_input_modal,
  draw_modal_list
};
use crate::app::{
  App,
  LoginField,
  ModalKind
};

pub(crate) fn draw_login(
  frame: &mut Frame,
  app: &App
) {
  let banner = banner_text(app);
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

  let password = Paragraph::new(
    "*".repeat(app.password.len())
  )
  .block(
    Block::default()
      .borders(Borders::ALL)
      .title("Password")
  )
  .style(password_style);

  frame
    .render_widget(username, chunks[0]);
  frame
    .render_widget(password, chunks[1]);

  let help = Paragraph::new(
    "Tab switches field. Enter logs \
     in."
  )
  .block(
    Block::default()
      .borders(Borders::ALL)
      .title("Help")
  );

  frame.render_widget(help, chunks[2]);

  draw_status_with_notice(
    frame,
    chunks[3],
    app.status.as_str(),
    banner
  );
}

pub(crate) fn draw_main(
  frame: &mut Frame,
  app: &App
) {
  let banner = banner_text(app);
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .margin(1)
    .constraints([
      Constraint::Length(3),
      Constraint::Min(3),
      Constraint::Length(3)
    ])
    .split(frame.area());

  let titles = [
    "Feeds",
    "Entries",
    "Favorites",
    "Folders",
    "Subscriptions"
  ]
  .iter()
  .map(|t| Line::from(*t))
  .collect::<Vec<_>>();

  let tabs = Tabs::new(titles)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title("Tabs")
    )
    .select(app.tab)
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
      draw_feed_view_list(
        frame,
        content[0],
        &app.feeds,
        &app.feeds_view,
        app.feeds_offset,
        app.feeds_page_size as usize,
        app.selected_feed,
        Some(&app.subscriptions),
        Some(&app.favorite_ids),
        Some(&app.feed_counts),
        "Feeds"
      );
      let selected = app
        .feeds_view
        .get(app.selected_feed)
        .and_then(|idx| {
          app.feeds.get(*idx)
        });
      let detail =
        selected.and_then(|feed| {
          app.feed_details.get(&feed.id)
        });
      draw_feed_detail(
        frame,
        content[1],
        selected,
        detail,
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
      let selected = app
        .entries
        .get(app.selected_entry);
      let detail =
        selected.and_then(|entry| {
          app
            .entry_details
            .get(&entry.id)
        });
      draw_entry_detail(
        frame, content[1], selected,
        detail
      );
    }
    | 2 => {
      draw_feed_list(
        frame,
        content[0],
        &app.favorites,
        app.favorites_offset,
        app.favorites_page_size
          as usize,
        app.selected_favorite,
        Some(&app.feed_counts),
        "Favorites"
      );
      let selected = app
        .favorites
        .get(app.selected_favorite);
      let detail =
        selected.and_then(|feed| {
          app.feed_details.get(&feed.id)
        });
      draw_feed_detail(
        frame,
        content[1],
        selected,
        detail,
        "Favorite Details"
      );
    }
    | 3 => {
      draw_folder_list(
        frame,
        content[0],
        &app.folders,
        app.selected_folder,
        app.folders_offset,
        app.folders_page_size as usize
      );
      draw_folder_feed_list(
        frame,
        content[1],
        &app.folder_feeds,
        app.folder_feeds_offset,
        app.feeds_page_size as usize,
        Some(&app.feed_counts),
        "Folder Feeds"
      );
    }
    | _ => {
      draw_feed_view_list(
        frame,
        content[0],
        &app.feeds,
        &app.subscriptions_view,
        app.subscriptions_offset,
        app.subscriptions_page_size
          as usize,
        app.selected_subscription,
        Some(&app.subscriptions),
        Some(&app.favorite_ids),
        Some(&app.feed_counts),
        "Subscriptions"
      );
      let selected = app
        .subscriptions_view
        .get(app.selected_subscription)
        .and_then(|idx| {
          app.feeds.get(*idx)
        });
      let detail =
        selected.and_then(|feed| {
          app.feed_details.get(&feed.id)
        });
      draw_feed_detail(
        frame,
        content[1],
        selected,
        detail,
        "Feed Details"
      );
    }
  }

  draw_status_with_notice(
    frame,
    chunks[2],
    app.status.as_str(),
    banner
  );

  if let Some(modal) = &app.modal {
    let title = match modal.kind {
      | ModalKind::Category => {
        "Select Category"
      }
      | ModalKind::Tag => "Select Tag",
      | ModalKind::Sort => "Sort Feeds",
      | ModalKind::FolderAssign => {
        "Assign to Folder"
      }
      | ModalKind::FolderUnassign => {
        "Remove from Folder"
      }
    };
    draw_modal_list(
      frame,
      title,
      &modal.options,
      modal.selected
    );
  }

  if let Some(input) = &app.input {
    draw_input_modal(
      frame,
      &input.title,
      &input.value
    );
  }
}

fn draw_status_with_notice(
  frame: &mut Frame,
  area: ratatui::layout::Rect,
  status_text: &str,
  notice: Option<(String, Style)>
) {
  if let Some((text, style)) = notice {
    let split = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([
        Constraint::Percentage(70),
        Constraint::Percentage(30)
      ])
      .split(area);

    let status =
      Paragraph::new(status_text)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .title("Status")
        )
        .wrap(Wrap {
          trim: true
        });

    let banner = Paragraph::new(text)
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title("Notice")
      )
      .style(style)
      .wrap(Wrap {
        trim: true
      });

    frame
      .render_widget(status, split[0]);
    frame
      .render_widget(banner, split[1]);
  } else {
    let status =
      Paragraph::new(status_text)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .title("Status")
        )
        .wrap(Wrap {
          trim: true
        });

    frame.render_widget(status, area);
  }
}

fn banner_text(
  app: &App
) -> Option<(String, Style)> {
  if let Some(err) = &app.error {
    return Some((
      format!("Error: {err}"),
      Style::default().fg(Color::Red)
    ));
  }

  if app.loading {
    return Some((
      "Loading...".to_string(),
      Style::default()
        .fg(Color::Yellow)
    ));
  }

  None
}
