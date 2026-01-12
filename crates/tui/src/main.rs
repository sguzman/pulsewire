use std::io::{
  self,
  Stdout
};
use std::time::{
  Duration,
  Instant
};

use anyhow::{
  Context,
  Result
};
use crossterm::event::{
  self,
  Event,
  KeyCode,
  KeyEvent,
  KeyModifiers
};
use crossterm::execute;
use crossterm::terminal::{
  EnterAlternateScreen,
  LeaveAlternateScreen,
  disable_raw_mode,
  enable_raw_mode
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
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
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]

struct FeedSummary {
  id:                String,
  url:               String,
  domain:            String,
  category:          String,
  base_poll_seconds: i64
}

#[derive(Debug, Deserialize, Clone)]

struct FolderRow {
  id:   i64,
  name: String
}

#[derive(Debug, Deserialize)]

struct TokenResponse {
  token: String
}

#[derive(Debug)]

enum Screen {
  Login,
  Main
}

#[derive(Debug, Clone, Copy)]

enum LoginField {
  Username,
  Password
}

struct App {
  screen:            Screen,
  focus:             LoginField,
  username:          String,
  password:          String,
  status:            String,
  token:             Option<String>,
  feeds:             Vec<FeedSummary>,
  favorites:         Vec<FeedSummary>,
  folders:           Vec<FolderRow>,
  tab:               usize,
  selected_feed:     usize,
  selected_favorite: usize,
  selected_folder:   usize,
  base_url:          String,
  client:            Client
}

impl App {
  fn new(
    base_url: String
  ) -> Result<Self> {
    let client =
      Client::builder().build()?;

    Ok(Self {
      screen: Screen::Login,
      focus: LoginField::Username,
      username: "admin".to_string(),
      password: "admin".to_string(),
      status: "Enter credentials. Tab \
               switches fields. Enter \
               to login."
        .to_string(),
      token: None,
      feeds: Vec::new(),
      favorites: Vec::new(),
      folders: Vec::new(),
      tab: 0,
      selected_feed: 0,
      selected_favorite: 0,
      selected_folder: 0,
      base_url,
      client
    })
  }

  fn handle_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    match self.screen {
      | Screen::Login => {
        self.handle_login_key(key)
      }
      | Screen::Main => {
        self.handle_main_key(key)
      }
    }
  }

  fn handle_login_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    match key {
      | KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        ..
      }
      | KeyEvent {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        ..
      } => return Ok(true),
      | KeyEvent {
        code: KeyCode::Tab,
        ..
      } => {
        self.focus = match self.focus {
          | LoginField::Username => {
            LoginField::Password
          }
          | LoginField::Password => {
            LoginField::Username
          }
        };
      }
      | KeyEvent {
        code: KeyCode::Enter,
        ..
      } => {
        self.login()?;
      }
      | KeyEvent {
        code: KeyCode::Backspace,
        ..
      } => {
        match self.focus {
          | LoginField::Username => {
            self.username.pop();
          }
          | LoginField::Password => {
            self.password.pop();
          }
        }
      }
      | KeyEvent {
        code: KeyCode::Char(ch),
        modifiers: KeyModifiers::NONE,
        ..
      } => {
        match self.focus {
          | LoginField::Username => {
            self.username.push(ch)
          }
          | LoginField::Password => {
            self.password.push(ch)
          }
        }
      }
      | _ => {}
    }

    Ok(false)
  }

  fn handle_main_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    match key {
      | KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        ..
      }
      | KeyEvent {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        ..
      } => return Ok(true),
      | KeyEvent {
        code: KeyCode::Char('1'),
        ..
      } => self.tab = 0,
      | KeyEvent {
        code: KeyCode::Char('2'),
        ..
      } => self.tab = 1,
      | KeyEvent {
        code: KeyCode::Char('3'),
        ..
      } => self.tab = 2,
      | KeyEvent {
        code: KeyCode::Left,
        ..
      } => {
        self.tab = (self.tab + 2) % 3;
      }
      | KeyEvent {
        code: KeyCode::Right,
        ..
      } => {
        self.tab = (self.tab + 1) % 3;
      }
      | KeyEvent {
        code: KeyCode::Char('r'),
        ..
      } => {
        self.refresh_tab()?;
      }
      | KeyEvent {
        code: KeyCode::Down,
        ..
      }
      | KeyEvent {
        code: KeyCode::Char('j'),
        ..
      } => {
        self.move_selection(1);
      }
      | KeyEvent {
        code: KeyCode::Up,
        ..
      }
      | KeyEvent {
        code: KeyCode::Char('k'),
        ..
      } => {
        self.move_selection(-1);
      }
      | _ => {}
    }

    Ok(false)
  }

  fn move_selection(
    &mut self,
    delta: i32
  ) {
    match self.tab {
      | 0 => {
        let len = self.feeds.len();

        self.selected_feed = move_index(
          self.selected_feed,
          len,
          delta
        );
      }
      | 1 => {
        let len = self.favorites.len();

        self.selected_favorite =
          move_index(
            self.selected_favorite,
            len,
            delta
          );
      }
      | _ => {
        let len = self.folders.len();

        self.selected_folder =
          move_index(
            self.selected_folder,
            len,
            delta
          );
      }
    }
  }

  fn login(&mut self) -> Result<()> {
    let url = format!(
      "{}/v1/auth/login",
      self.base_url
    );

    let body = serde_json::json!({
        "username": &self.username,
        "password": &self.password,
    });

    let resp = self
      .client
      .post(url)
      .json(&body)
      .send()
      .context(
        "login request failed"
      )?;

    if !resp.status().is_success() {
      let msg = resp
        .text()
        .unwrap_or_else(|_| {
          "login failed".to_string()
        });

      self.status =
        format!("Login failed: {msg}");

      return Ok(());
    }

    let token = resp
      .json::<TokenResponse>()?
      .token;

    self.token = Some(token);

    self.screen = Screen::Main;

    self.status = "Logged in. Press r \
                   to refresh."
      .to_string();

    self.refresh_all()?;

    Ok(())
  }

  fn refresh_all(
    &mut self
  ) -> Result<()> {
    self.refresh_feeds()?;

    self.refresh_favorites()?;

    self.refresh_folders()?;

    Ok(())
  }

  fn refresh_tab(
    &mut self
  ) -> Result<()> {
    match self.tab {
      | 0 => self.refresh_feeds(),
      | 1 => self.refresh_favorites(),
      | _ => self.refresh_folders()
    }
  }

  fn refresh_feeds(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/feeds",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "feeds request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load feeds ({})",
        resp.status()
      );

      return Ok(());
    }

    self.feeds = resp.json().context(
      "failed to parse feeds"
    )?;

    if self.selected_feed
      >= self.feeds.len()
    {
      self.selected_feed = 0;
    }

    self.status = format!(
      "Loaded {} feeds",
      self.feeds.len()
    );

    Ok(())
  }

  fn refresh_favorites(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/favorites",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "favorites request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load favorites ({})",
        resp.status()
      );

      return Ok(());
    }

    self.favorites =
      resp.json().context(
        "failed to parse favorites"
      )?;

    if self.selected_favorite
      >= self.favorites.len()
    {
      self.selected_favorite = 0;
    }

    self.status = format!(
      "Loaded {} favorites",
      self.favorites.len()
    );

    Ok(())
  }

  fn refresh_folders(
    &mut self
  ) -> Result<()> {
    let token = self
      .token
      .as_deref()
      .unwrap_or_default();

    let url = format!(
      "{}/v1/folders",
      self.base_url
    );

    let resp = self
      .client
      .get(url)
      .bearer_auth(token)
      .send()
      .context(
        "folders request failed"
      )?;

    if !resp.status().is_success() {
      self.status = format!(
        "Failed to load folders ({})",
        resp.status()
      );

      return Ok(());
    }

    self.folders =
      resp.json().context(
        "failed to parse folders"
      )?;

    if self.selected_folder
      >= self.folders.len()
    {
      self.selected_folder = 0;
    }

    self.status = format!(
      "Loaded {} folders",
      self.folders.len()
    );

    Ok(())
  }
}

fn move_index(
  current: usize,
  len: usize,
  delta: i32
) -> usize {
  if len == 0 {
    return 0;
  }

  let max =
    len.saturating_sub(1) as i32;

  let next = (current as i32 + delta)
    .clamp(0, max);

  next as usize
}

fn main() -> Result<()> {
  let base_url =
    std::env::var("FEEDRV3_SERVER_URL")
      .unwrap_or_else(|_| {
        "http://localhost:8091"
          .to_string()
      });

  enable_raw_mode()?;

  let mut stdout = io::stdout();

  execute!(
    stdout,
    EnterAlternateScreen
  )?;

  let backend =
    CrosstermBackend::new(stdout);

  let mut terminal =
    Terminal::new(backend)?;

  let mut app = App::new(base_url)?;

  app.status = "Attempting auto-login \
                as admin..."
    .to_string();

  if let Err(err) = app.login() {
    app.status = format!(
      "Auto-login failed: {err}"
    );

    app.screen = Screen::Login;
  }

  let tick_rate =
    Duration::from_millis(200);

  let mut last_tick = Instant::now();

  let res = run_app(
    &mut terminal,
    &mut app,
    tick_rate,
    &mut last_tick
  );

  disable_raw_mode()?;

  execute!(
    terminal.backend_mut(),
    LeaveAlternateScreen
  )?;

  terminal.show_cursor()?;

  res
}

fn run_app(
  terminal: &mut Terminal<
    CrosstermBackend<Stdout>
  >,
  app: &mut App,
  tick_rate: Duration,
  last_tick: &mut Instant
) -> Result<()> {
  loop {
    terminal.draw(|frame| {
      match app.screen {
        | Screen::Login => {
          draw_login(frame, app)
        }
        | Screen::Main => {
          draw_main(frame, app)
        }
      }
    })?;

    let timeout = tick_rate
      .saturating_sub(
        last_tick.elapsed()
      );

    if event::poll(timeout)? {
      if let Event::Key(key) =
        event::read()?
      {
        if app.handle_key(key)? {
          return Ok(());
        }
      }
    }

    if last_tick.elapsed() >= tick_rate
    {
      *last_tick = Instant::now();
    }
  }
}

fn draw_login(
  frame: &mut ratatui::Frame,
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

fn draw_main(
  frame: &mut ratatui::Frame,
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
    "Favorites (2)",
    "Folders (3)"
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
      draw_feed_list(
        frame,
        content[0],
        &app.favorites,
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
  frame: &mut ratatui::Frame,
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

fn draw_folder_detail(
  frame: &mut ratatui::Frame,
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
  frame: &mut ratatui::Frame,
  area: Rect,
  feeds: &[FeedSummary],
  selected: usize,
  title: &str
) {
  let items = feeds
    .iter()
    .map(|feed| {
      let label = format!(
        "{} [{}]",
        feed.id, feed.domain
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

fn draw_folder_list(
  frame: &mut ratatui::Frame,
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
  let mut state = ratatui::widgets::ListState::default();

  if len > 0 {
    state.select(Some(
      selected
        .min(len.saturating_sub(1))
    ));
  }

  state
}
