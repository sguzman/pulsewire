use anyhow::Result;
use crossterm::event::{
  KeyCode,
  KeyEvent
};

use super::{
  App,
  ModalKind,
  ModalState,
  SortMode
};

impl App {
  pub(super) fn handle_modal_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    let move_down = self.key_matches(
      &self.keys.move_down,
      key
    );
    let move_up = self.key_matches(
      &self.keys.move_up,
      key
    );
    let go_top = self.key_matches(
      &self.keys.go_top,
      key
    );
    let go_bottom = self.key_matches(
      &self.keys.go_bottom,
      key
    );

    let Some(modal) =
      self.modal.as_mut()
    else {
      return Ok(false);
    };

    if key.code == KeyCode::Esc {
      self.modal = None;
      return Ok(false);
    }

    if key.code == KeyCode::Enter {
      let selection = modal.selected;
      let option = modal
        .options
        .get(selection)
        .cloned()
        .unwrap_or_default();

      match modal.kind {
        | ModalKind::Category => {
          if selection == 0 {
            self.filter_category = None;
          } else {
            self.filter_category =
              Some(option);
          }
          self.rebuild_views();
          self.status = format!(
            "Filters: {}",
            self.filter_summary()
          );
        }
        | ModalKind::Tag => {
          if selection == 0 {
            self.filter_tag = None;
          } else {
            self.filter_tag =
              Some(option);
          }
          self.rebuild_views();
          self.status = format!(
            "Filters: {}",
            self.filter_summary()
          );
        }
        | ModalKind::Sort => {
          self.sort_mode =
            match selection {
              | 1 => SortMode::Total,
              | 2 => SortMode::Ratio,
              | 3 => SortMode::Recent,
              | _ => SortMode::Unread
            };
          self.apply_sort();
        }
        | ModalKind::FolderAssign => {
          if let (
            Some(indices),
            Some(feed_id)
          ) = (
            modal
              .folder_indices
              .as_ref(),
            modal.feed_id.clone()
          ) {
            if let Some(folder_index) =
              indices.get(selection)
            {
              if let Some(folder) = self
                .folders
                .get(*folder_index)
              {
                self.queue_assign_folder_feed(
                  folder.id,
                  feed_id
                );
              }
            }
          }
        }
        | ModalKind::FolderUnassign => {
          if let (
            Some(indices),
            Some(feed_id)
          ) = (
            modal
              .folder_indices
              .as_ref(),
            modal.feed_id.clone()
          ) {
            if let Some(folder_index) =
              indices.get(selection)
            {
              if let Some(folder) = self
                .folders
                .get(*folder_index)
              {
                self.queue_unassign_folder_feed(
                  folder.id,
                  feed_id
                );
              }
            }
          }
        }
      }

      self.modal = None;
      return Ok(false);
    }

    if move_down {
      if modal.selected + 1
        < modal.options.len()
      {
        modal.selected += 1;
      }
      return Ok(false);
    }

    if move_up {
      if modal.selected > 0 {
        modal.selected -= 1;
      }
      return Ok(false);
    }

    if go_top {
      modal.selected = 0;
      return Ok(false);
    }

    if go_bottom {
      if !modal.options.is_empty() {
        modal.selected =
          modal.options.len() - 1;
      }
      return Ok(false);
    }

    Ok(false)
  }

  pub(super) fn open_category_menu(
    &mut self
  ) {
    let mut options =
      Vec::with_capacity(
        self.categories.len() + 1
      );
    options.push("All".to_string());
    options.extend(
      self.categories.iter().cloned()
    );

    let selected = self
      .filter_category
      .as_ref()
      .and_then(|cat| {
        options
          .iter()
          .position(|v| v == cat)
      })
      .unwrap_or(0);

    self.modal = Some(ModalState {
      kind: ModalKind::Category,
      options,
      selected,
      folder_indices: None,
      feed_id: None
    });
  }

  pub(super) fn open_tag_menu(
    &mut self
  ) {
    let mut options =
      Vec::with_capacity(
        self.tags.len() + 1
      );
    options.push("All".to_string());
    options.extend(
      self.tags.iter().cloned()
    );

    let selected = self
      .filter_tag
      .as_ref()
      .and_then(|tag| {
        options
          .iter()
          .position(|v| v == tag)
      })
      .unwrap_or(0);

    self.modal = Some(ModalState {
      kind: ModalKind::Tag,
      options,
      selected,
      folder_indices: None,
      feed_id: None
    });
  }

  pub(super) fn open_sort_menu(
    &mut self
  ) {
    let options = vec![
      "Unread count".to_string(),
      "Total count".to_string(),
      "Unread/Total ratio".to_string(),
      "Most recent".to_string(),
    ];

    let selected = match self.sort_mode
    {
      | SortMode::Unread => 0,
      | SortMode::Total => 1,
      | SortMode::Ratio => 2,
      | SortMode::Recent => 3
    };

    self.modal = Some(ModalState {
      kind: ModalKind::Sort,
      options,
      selected,
      folder_indices: None,
      feed_id: None
    });
  }

  pub(super) fn open_folder_menu(
    &mut self,
    feed_id: String,
    assign: bool
  ) {
    if self.folders.is_empty() {
      self.status = "No folders \
                     available"
        .to_string();
      return;
    }

    let options = self
      .folders
      .iter()
      .map(|folder| folder.name.clone())
      .collect::<Vec<_>>();

    let folder_indices =
      (0..self.folders.len())
        .collect::<Vec<_>>();

    self.modal = Some(ModalState {
      kind: if assign {
        ModalKind::FolderAssign
      } else {
        ModalKind::FolderUnassign
      },
      options,
      selected: 0,
      folder_indices: Some(
        folder_indices
      ),
      feed_id: Some(feed_id)
    });
  }
}
