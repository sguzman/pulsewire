use std::path::Path;

use tokio::fs;

use super::ConfigError;

pub(crate) async fn load_schema(
  schema_dir: &Path,
  filename: &str
) -> Result<String, ConfigError> {
  let path = schema_dir.join(filename);

  let content =
    fs::read_to_string(&path)
      .await
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "schema file missing at {}: \
           {e}",
          path.display()
        ))
      })?;

  Ok(content)
}

pub(crate) fn validate_toml(
  schema_json: &str,
  toml_text: &str,
  context: &str
) -> Result<(), ConfigError> {
  let schema_value: serde_json::Value =
    serde_json::from_str(schema_json)
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "schema parse error: {e}"
        ))
      })?;

  let schema =
    jsonschema::validator_for(
      &schema_value
    )
    .map_err(|e| {
      ConfigError::Invalid(format!(
        "schema compile error: {e}"
      ))
    })?;

  let toml_value: toml::Value =
    toml::from_str(toml_text)?;

  let json_value =
    serde_json::to_value(toml_value)
      .map_err(|e| {
        ConfigError::Invalid(format!(
          "schema input error: {e}"
        ))
      })?;

  // JSON Schema errors can quote the
  // rejected value, which may contain a
  // password, cookie, or API token.
  // Report only context.
  if schema
    .iter_errors(&json_value)
    .next()
    .is_some()
  {
    return Err(ConfigError::Invalid(
      format!(
        "schema validation failed for \
         {context} (values omitted)"
      )
    ));
  }

  Ok(())
}

#[cfg(test)]
mod redaction_tests {
  use super::*;

  #[test]
  fn schema_errors_do_not_echo_password_values()
   {
    let schema = r#"{"type":"object","properties":{"password":{"type":"integer"}}}"#;
    let input =
      "password = \
       \"pw023-config-secret-sentinel\\
       "";
    let err = validate_toml(
      schema, input, "test"
    )
    .expect_err(
      "string is not integer"
    );
    for message in [
      err.to_string(),
      format!("{err:?}")
    ] {
      assert!(
        message
          .contains("values omitted")
      );
      assert!(!message.contains(
        "pw023-config-secret-sentinel"
      ));
    }
  }
}
