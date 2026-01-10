use std::path::Path;

use tokio::fs;

use super::ConfigError;

pub(crate) async fn load_schema(
    schema_dir: &Path,
    filename: &str,
) -> Result<String, ConfigError> {
    let path = schema_dir.join(filename);
    let content = fs::read_to_string(&path).await.map_err(|e| {
        ConfigError::Invalid(format!(
            "schema file missing at {}: {e}",
            path.display()
        ))
    })?;
    Ok(content)
}

pub(crate) fn validate_toml(
    schema_json: &str,
    toml_text: &str,
    context: &str,
) -> Result<(), ConfigError> {
    let schema_value: serde_json::Value = serde_json::from_str(schema_json)
        .map_err(|e| ConfigError::Invalid(format!("schema parse error: {e}")))?;
    let schema = jsonschema::validator_for(&schema_value)
        .map_err(|e| ConfigError::Invalid(format!("schema compile error: {e}")))?;

    let toml_value: toml::Value = toml::from_str(toml_text)?;
    let json_value = serde_json::to_value(toml_value)
        .map_err(|e| ConfigError::Invalid(format!("schema input error: {e}")))?;

    let mut errors = schema.iter_errors(&json_value);
    if let Some(err) = errors.next() {
        let mut messages = vec![err.to_string()];
        for e in errors.take(4) {
            messages.push(e.to_string());
        }
        return Err(ConfigError::Invalid(format!(
            "schema validation failed for {context}: {}",
            messages.join("; ")
        )));
    }

    Ok(())
}
