//! Resolve PostgreSQL passwords without
//! forcing secret bytes into the Docker
//! container environment.
//!
//! The *_FILE override is fail-closed:
//! unreadable or invalid files never
//! fall back to a less secure source.
//! Avoid returning file contents or
//! secret paths in error messages.

use std::fs;
use std::path::Path;

const MAX_SECRET_BYTES: usize = 4096;

pub fn postgres_password(
  configured: String
) -> Result<String, &'static str> {
  let file = std::env::var(
    "PULSEWIRE_POSTGRES_PASSWORD_FILE"
  )
  .ok()
  .filter(|value| !value.is_empty());

  let direct = std::env::var(
    "PULSEWIRE_POSTGRES_PASSWORD"
  )
  .ok()
  .filter(|value| !value.is_empty());

  resolve_sources(
    configured,
    file.as_deref(),
    direct.as_deref()
  )
}

fn resolve_sources(
  configured: String,
  file: Option<&str>,
  direct: Option<&str>
) -> Result<String, &'static str> {
  if file.is_some() && direct.is_some() {
    return Err(
      "use only one PostgreSQL \
       password source: environment \
       or secret file"
    );
  }

  if let Some(file) = file {
    let path = Path::new(file);
    if !path.is_file() {
      return Err(
        "PostgreSQL password file \
         missing or not a regular file"
      );
    }

    let mut content =
      fs::read_to_string(path)
        .map_err(|_| {
          "PostgreSQL password file \
           could not be read"
        })?;

    // Support secrets created by
    // printf or a standard text editor.
    if content.ends_with('\n') {
      content.pop();
      if content.ends_with('\r') {
        content.pop();
      }
    }

    if content.is_empty()
      || content.len() > MAX_SECRET_BYTES
      || content.chars().any(|ch| {
        matches!(ch, '\n' | '\r' | '\0')
      })
    {
      return Err(
        "PostgreSQL password file \
         has invalid content"
      );
    }

    return Ok(content);
  }

  if let Some(direct) = direct {
    return Ok(direct.to_string());
  }

  Ok(configured)
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::*;

  fn fixture_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(
      format!(
        "pulsewire-password-file-\
         {}-{label}.txt",
        std::process::id()
      )
    )
  }

  #[test]
  fn secret_file_overrides_config_and_newline_is_removed()
   {
    let path =
      fixture_path("valid");
    fs::write(&path, b"from-file\r\n")
      .expect("fixture");
    let result = resolve_sources(
      "from-toml".into(),
      path.to_str(),
      None
    )
    .expect("file secret");
    assert_eq!(result, "from-file");
    fs::remove_file(path)
      .expect("remove fixture");
  }

  #[test]
  fn conflicting_sources_are_rejected()
   {
    let result = resolve_sources(
      "from-toml".into(),
      Some("/invalid/path"),
      Some("from-env")
    );
    assert!(result.is_err());
    assert!(!result.unwrap_err().contains(
      "from-env"
    ));
  }

  #[test]
  fn invalid_file_never_falls_back()
   {
    let path =
      fixture_path("invalid");
    fs::write(
      &path,
      b"very-secret-value\nsecond-line"
    )
    .expect("fixture");
    let result = resolve_sources(
      "from-toml".into(),
      path.to_str(),
      None
    );
    assert!(result.is_err());
    assert!(!result.unwrap_err().contains(
      "very-secret-value"
    ));
    fs::remove_file(path)
      .expect("remove fixture");

    let missing = resolve_sources(
      "from-toml".into(),
      Some("/definitely/missing/file"),
      None
    );
    assert!(missing.is_err());
  }

  #[test]
  fn existing_environment_and_private_config_still_work()
   {
    assert_eq!(
      resolve_sources(
        "from-toml".into(),
        None,
        Some("from-env")
      )
      .expect("env"),
      "from-env"
    );
    assert_eq!(
      resolve_sources(
        "from-toml".into(),
        None,
        None
      )
      .expect("config"),
      "from-toml"
    );
  }
}
