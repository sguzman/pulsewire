mod login;
mod main;
mod navigation;
mod text;

use anyhow::Result;
use crossterm::event::KeyEvent;

use super::{
  App,
  Screen
};

impl App {
  pub(crate) fn handle_key(
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
}
