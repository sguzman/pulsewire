use anyhow::{
  Context,
  Result
};

use super::super::App;
use super::super::util::ensure_offset;

impl App {
  pub(crate) fn refresh_favorites(
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
    self.favorite_ids = self
      .favorites
      .iter()
      .map(|row| row.id.clone())
      .collect();

    self.sort_favorites();

    if self.selected_favorite
      >= self.favorites.len()
    {
      self.selected_favorite = 0;
      self.favorites_offset = 0;
    }

    self.favorites_offset =
      ensure_offset(
        self.selected_favorite,
        self.favorites_offset,
        self.favorites_page_size
          as usize,
        self.favorites.len()
      );

    self.status = format!(
      "Loaded {} favorites",
      self.favorites.len()
    );

    Ok(())
  }
}
