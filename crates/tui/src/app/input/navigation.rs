use crossterm::event::KeyEvent;

use super::super::App;
use super::super::util::move_index;
use crate::config::KeyBinding;

impl App {
  pub(crate) fn key_matches(
    &self,
    binding: &KeyBinding,
    key: KeyEvent
  ) -> bool {
    key.code == binding.code
      && key.modifiers
        == binding.modifiers
  }

  pub(super) fn move_selection(
    &mut self,
    delta: i32
  ) {
    match self.tab {
      | 0 => {
        let len = self.feeds_view.len();
        self.selected_feed = move_index(
          self.selected_feed,
          len,
          delta
        );
        self.ensure_visible_for_tab();
      }
      | 1 => {
        let len = self.entries.len();
        self.selected_entry =
          move_index(
            self.selected_entry,
            len,
            delta
          );
      }
      | 2 => {
        let len = self.favorites.len();
        self.selected_favorite =
          move_index(
            self.selected_favorite,
            len,
            delta
          );
        self.ensure_visible_for_tab();
      }
      | 3 => {
        let len = self.folders.len();
        let before =
          self.selected_folder;
        self.selected_folder =
          move_index(
            self.selected_folder,
            len,
            delta
          );
        self.ensure_visible_for_tab();
        if before
          != self.selected_folder
        {
          self.request_folder_feeds();
        }
      }
      | _ => {
        let len =
          self.subscriptions_view.len();
        self.selected_subscription =
          move_index(
            self.selected_subscription,
            len,
            delta
          );
        self.ensure_visible_for_tab();
      }
    }

    self.prefetch_selection_details();
  }

  pub(super) fn jump_top(&mut self) {
    match self.tab {
      | 0 => {
        self.selected_feed = 0;
        self.ensure_visible_for_tab();
      }
      | 1 => {
        self.selected_entry = 0;
      }
      | 2 => {
        self.selected_favorite = 0;
        self.ensure_visible_for_tab();
      }
      | 3 => {
        self.selected_folder = 0;
        self.ensure_visible_for_tab();
        self.request_folder_feeds();
      }
      | _ => {
        self.selected_subscription = 0;
        self.ensure_visible_for_tab();
      }
    }

    self.prefetch_selection_details();
  }

  pub(super) fn jump_middle(&mut self) {
    match self.tab {
      | 0 => {
        if !self.feeds_view.is_empty() {
          self.selected_feed =
            self.feeds_view.len() / 2;
          self.ensure_visible_for_tab();
        }
      }
      | 1 => {
        if !self.entries.is_empty() {
          self.selected_entry =
            self.entries.len() / 2;
        }
      }
      | 2 => {
        if !self.favorites.is_empty() {
          self.selected_favorite =
            self.favorites.len() / 2;
          self.ensure_visible_for_tab();
        }
      }
      | 3 => {
        if !self.folders.is_empty() {
          self.selected_folder =
            self.folders.len() / 2;
          self.ensure_visible_for_tab();
          self.request_folder_feeds();
        }
      }
      | _ => {
        if !self
          .subscriptions_view
          .is_empty()
        {
          self.selected_subscription =
            self
              .subscriptions_view
              .len()
              / 2;
          self.ensure_visible_for_tab();
        }
      }
    }

    self.prefetch_selection_details();
  }

  pub(super) fn jump_bottom(&mut self) {
    match self.tab {
      | 0 => {
        if !self.feeds_view.is_empty() {
          self.selected_feed =
            self.feeds_view.len() - 1;
          self.ensure_visible_for_tab();
        }
      }
      | 1 => {
        if !self.entries.is_empty() {
          self.selected_entry =
            self.entries.len() - 1;
        }
      }
      | 2 => {
        if !self.favorites.is_empty() {
          self.selected_favorite =
            self.favorites.len() - 1;
          self.ensure_visible_for_tab();
        }
      }
      | 3 => {
        if !self.folders.is_empty() {
          self.selected_folder =
            self.folders.len() - 1;
          self.ensure_visible_for_tab();
          self.request_folder_feeds();
        }
      }
      | _ => {
        if !self
          .subscriptions_view
          .is_empty()
        {
          self.selected_subscription =
            self
              .subscriptions_view
              .len()
              - 1;
          self.ensure_visible_for_tab();
        }
      }
    }

    self.prefetch_selection_details();
  }
}
