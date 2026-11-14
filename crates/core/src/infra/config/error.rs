#[derive(thiserror::Error)]
pub enum ConfigError {
  #[error("io: {0}")]
  Io(#[from] std::io::Error),
  #[error(
    "toml: invalid input (source \
     omitted to protect secrets)"
  )]
  Toml,
  #[error("invalid config: {0}")]
  Invalid(String)
}

// Discard the parser error: it may
// contain source snippets and
// must not survive via Error::source()
// or diagnostic formatting.
impl From<toml::de::Error>
  for ConfigError
{
  fn from(_: toml::de::Error) -> Self {
    Self::Toml
  }
}

impl std::fmt::Debug for ConfigError {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>
  ) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

#[cfg(test)]
mod redaction_tests {
  use super::*;

  #[test]
  fn toml_error_hides_source_in_display_and_debug()
   {
    let malformed =
      "password = \
       \"pw023-config-secret-sentinel";
    let parse_error = toml::from_str::<
      toml::Value
    >(malformed)
    .expect_err(
      "unterminated string is invalid"
    );
    let err =
      ConfigError::from(parse_error);
    for message in [
      err.to_string(),
      format!("{err:?}")
    ] {
      assert!(
        message
          .contains("source omitted")
      );
      assert!(!message.contains(
        "pw023-config-secret-sentinel"
      ));
    }
  }
}
