mod app;
mod config;
mod models;
mod ui;

use std::io::{
  self,
  Stdout
};
use std::path::PathBuf;
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
  Event
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

use crate::app::{
  App,
  Screen
};
use crate::config::{
  TuiConfig,
  default_config_path
};
use crate::ui::{
  draw_login,
  draw_main
};

fn main() -> Result<()> {
  let config_path =
    resolve_config_path();
  let config =
    TuiConfig::load(&config_path)
      .with_context(|| {
        format!(
          "load config: {}",
          config_path.display()
        )
      })?;
  let keys = config
    .resolved_keybindings()
    .with_context(|| {
      "resolve keybindings"
    })?;

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

  let mut app =
    App::new(&config, keys)?;

  if config.auth.auto_login {
    app.status = "Attempting \
                  auto-login..."
      .to_string();

    if let Err(err) = app.login() {
      app.status = format!(
        "Auto-login failed: {err}"
      );
      app.screen = Screen::Login;
    }
  }

  let tick_rate = Duration::from_millis(
    config.ui.refresh_interval_ms
  );

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

fn resolve_config_path() -> PathBuf {
  if let Some(path) =
    std::env::args().nth(1)
  {
    return PathBuf::from(path);
  }

  if let Ok(path) =
    std::env::var("FEEDRV3_TUI_CONFIG")
  {
    return PathBuf::from(path);
  }

  default_config_path()
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
