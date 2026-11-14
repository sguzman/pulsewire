//! Resolve service schema files from
//! runtime paths, never from a
//! build machine's source directory
//! baked into an installed binary.

/// Compatibility symbol only: the
/// build-time source root is not
/// consulted when locating schemas at
/// runtime.
pub const SCHEMAS_ROOT: &str =
  env!("CARGO_MANIFEST_DIR");

use std::path::{
  Path,
  PathBuf
};

/// Locate a service's schema directory.
///
/// SCHEMAS_DIR always wins (even when
/// nonexistent, to fail explicitly).
/// Otherwise prefer the XDG data
/// installation, executable-adjacent
/// installation, then a local workspace
/// tree for cargo run in a checkout.
pub fn schema_dir(
  service: &str
) -> String {
  let override_dir =
    std::env::var("SCHEMAS_DIR").ok();
  let xdg_data =
    std::env::var("XDG_DATA_HOME")
      .ok()
      .map(PathBuf::from);
  let home = std::env::var("HOME")
    .ok()
    .map(PathBuf::from);
  let executable =
    std::env::current_exe().ok();
  let cwd =
    std::env::current_dir().ok();

  resolve_schema_dir(
    service,
    override_dir.as_deref(),
    xdg_data.as_deref(),
    home.as_deref(),
    executable.as_deref(),
    cwd.as_deref()
  )
  .to_string_lossy()
  .into_owned()
}

fn resolve_schema_dir(
  service: &str,
  override_dir: Option<&str>,
  xdg_data: Option<&Path>,
  home: Option<&Path>,
  executable: Option<&Path>,
  cwd: Option<&Path>
) -> PathBuf {
  if let Some(value) = override_dir
    .filter(|value| {
      !value.trim().is_empty()
    })
  {
    return PathBuf::from(value);
  }

  let mut candidates = Vec::new();
  if let Some(data) = xdg_data
    .filter(|path| path.is_absolute())
  {
    candidates.push(
      data
        .join("pulsewire/schemas")
        .join(service)
    );
  } else if let Some(home) = home
    .filter(|path| path.is_absolute())
  {
    candidates.push(
      home
        .join(
          ".local/share/pulsewire/\
           schemas"
        )
        .join(service)
    );
  }

  if let Some(bin_dir) =
    executable.and_then(Path::parent)
  {
    candidates.push(
      bin_dir
        .join(
          "../share/pulsewire/schemas"
        )
        .join(service)
    );
    candidates.push(
      bin_dir
        .join("schemas")
        .join(service)
    );
  }
  if let Some(cwd) = cwd {
    candidates.push(
      cwd
        .join("crates/schemas")
        .join(service)
    );
    candidates.push(
      cwd.join("schemas").join(service)
    );
    candidates.push(cwd.join(service));
  }

  candidates
    .iter()
    .find(|path| path.is_dir())
    .cloned()
    .or_else(|| {
      candidates.into_iter().next()
    })
    .unwrap_or_else(|| {
      PathBuf::from("schemas")
        .join(service)
    })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn temporary_root(
    label: &str
  ) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
      "pulsewire-schema-dir-test-{}-{label}",
      std::process::id()
    ));
    let _ =
      std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)
      .expect("test root");
    root
  }

  #[test]
  fn explicit_override_wins_even_if_nonexistent()
   {
    let root =
      temporary_root("override");
    let override_path =
      root.join("private/schemas");
    let chosen = resolve_schema_dir(
      "fetcher",
      override_path.to_str(),
      None,
      None,
      None,
      Some(&root)
    );
    assert_eq!(chosen, override_path);
    let _ =
      std::fs::remove_dir_all(root);
  }

  #[test]
  fn xdg_installation_precedes_workspace()
   {
    let root = temporary_root("xdg");
    let xdg = root.join("xdg");
    let workspace =
      root.join("workspace");
    let expected = xdg.join(
      "pulsewire/schemas/fetcher"
    );
    std::fs::create_dir_all(&expected)
      .expect("xdg schema tree");
    std::fs::create_dir_all(
      workspace
        .join("crates/schemas/fetcher")
    )
    .expect("workspace schema tree");
    let chosen = resolve_schema_dir(
      "fetcher",
      None,
      Some(&xdg),
      None,
      None,
      Some(&workspace)
    );
    assert_eq!(chosen, expected);
    let _ =
      std::fs::remove_dir_all(root);
  }

  #[test]
  fn installed_binary_uses_nearby_share_directory()
   {
    let root =
      temporary_root("executable");
    let exe =
      root.join("bin/pulsewire-server");
    std::fs::create_dir_all(
      root.join("bin")
    )
    .expect("binary directory");
    let expected = root.join(
      "share/pulsewire/schemas/server"
    );
    std::fs::create_dir_all(&expected)
      .expect(
        "installation schema tree"
      );
    let chosen = resolve_schema_dir(
      "server",
      None,
      None,
      None,
      Some(&exe),
      None
    );
    assert_eq!(
      chosen,
      exe.parent().unwrap().join(
        "../share/pulsewire/schemas/\
         server"
      )
    );
    let _ =
      std::fs::remove_dir_all(root);
  }

  #[test]
  fn absent_schema_reports_expected_xdg_location()
   {
    let root =
      temporary_root("missing");
    let xdg = root.join("xdg");
    let chosen = resolve_schema_dir(
      "server",
      None,
      Some(&xdg),
      None,
      None,
      None
    );
    assert_eq!(
      chosen,
      xdg.join(
        "pulsewire/schemas/server"
      )
    );
    let _ =
      std::fs::remove_dir_all(root);
  }
}
