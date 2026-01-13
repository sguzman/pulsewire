use anyhow::Result;
use crossterm::event::{
  KeyCode,
  KeyEvent
};

use super::super::{
  App,
  InputKind,
  InputState
};

impl App {
  pub(super) fn handle_input_key(
    &mut self,
    key: KeyEvent
  ) -> Result<bool> {
    match key.code {
      | KeyCode::Esc => {
        self.input = None;
        return Ok(false);
      }
      | KeyCode::Enter => {
        let input =
          match self.input.take() {
            | Some(input) => input,
            | None => return Ok(false)
          };
        let value = input.value.trim();
        match input.kind {
          | InputKind::FolderCreate => {
            if value.is_empty() {
              self.status =
                "Folder name is required"
                  .to_string();
              self.input = Some(input);
              return Ok(false);
            }
            self.queue_create_folder(
              value.to_string()
            );
          }
          | InputKind::FolderRename {
            folder_id
          } => {
            if value.is_empty() {
              self.status =
                "Folder name is required"
                  .to_string();
              self.input = Some(input);
              return Ok(false);
            }
            self.queue_rename_folder(
              folder_id,
              value.to_string()
            );
          }
          | InputKind::EntriesSearch => {
            self.open_search_entries(
              value.to_string()
            )?;
          }
        }
        return Ok(false);
      }
      | KeyCode::Backspace => {
        if let Some(input) =
          self.input.as_mut()
        {
          input.value.pop();
        }
      }
      | KeyCode::Char(ch) => {
        if let Some(input) =
          self.input.as_mut()
        {
          input.value.push(ch);
        }
      }
      | _ => {}
    }

    Ok(false)
  }

  pub(super) fn open_folder_create_input(
    &mut self
  ) {
    self.input = Some(InputState {
      kind:  InputKind::FolderCreate,
      title: "New folder".to_string(),
      value: String::new()
    });
  }

  pub(super) fn open_folder_rename_input(
    &mut self
  ) {
    let folder = self
      .folders
      .get(self.selected_folder)
      .cloned();

    let Some(folder) = folder else {
      self.status =
        "No folder selected"
          .to_string();
      return;
    };

    self.input = Some(InputState {
      kind:  InputKind::FolderRename {
        folder_id: folder.id
      },
      title: "Rename folder"
        .to_string(),
      value: folder.name
    });
  }

  pub(super) fn open_search_input(
    &mut self
  ) {
    self.input = Some(InputState {
      kind:  InputKind::EntriesSearch,
      title: "Search entries"
        .to_string(),
      value: String::new()
    });
  }
}
