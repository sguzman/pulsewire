//! Provides the shared schema directory
//! used by all binaries.

/// Absolute path to the workspace
/// schema root.
pub const SCHEMAS_ROOT: &str =
  env!("CARGO_MANIFEST_DIR");

/// Helper to build the absolute path
/// for a service schema directory.
pub fn schema_dir(
  service: &str
) -> String {
  std::env::var("SCHEMAS_DIR")
    .ok()
    .filter(|path| {
      !path.trim().is_empty()
    })
    .unwrap_or_else(|| {
      format!(
        "{SCHEMAS_ROOT}/{service}"
      )
    })
}
