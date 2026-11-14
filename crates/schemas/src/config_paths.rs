//! Find Pulsewire configuration at runtime.
//! Explicit paths win; installation and
//! development locations are fallbacks.

use std::path::{Path, PathBuf};

/// A CLI override, then an environment
/// override, then an installed/user
/// config, then development defaults.
pub fn config_path(
  service: &str,
  cli: Option<&str>,
  environment_name: &str
) -> PathBuf {
  let environment = std::env::var(
    environment_name
  ).ok();
  let xdg = std::env::var(
    "XDG_CONFIG_HOME"
  ).ok().map(PathBuf::from);
  let home = std::env::var(
    "HOME"
  ).ok().map(PathBuf::from);
  let executable =
    std::env::current_exe().ok();
  let cwd = std::env::current_dir().ok();

  resolve_config_path(
    service,
    cli,
    environment.as_deref(),
    xdg.as_deref(),
    home.as_deref(),
    executable.as_deref(),
    cwd.as_deref()
  )
}

fn resolve_config_path(
  service: &str,
  cli: Option<&str>,
  environment: Option<&str>,
  xdg: Option<&Path>,
  home: Option<&Path>,
  executable: Option<&Path>,
  cwd: Option<&Path>
) -> PathBuf {
  // User-provided paths are authoritative
  // even if they do not exist. A typo
  // must fail rather than silently
  // selecting another configuration.
  if let Some(path) = cli {
    return PathBuf::from(path);
  }
  if let Some(path) = environment
    .filter(|value| {
      !value.trim().is_empty()
    })
  {
    return PathBuf::from(path);
  }

  let mut candidates = Vec::new();
  let user_home = home
    .filter(|path| path.is_absolute());
  if let Some(data) = xdg
    .filter(|path| path.is_absolute())
  {
    candidates.push(
      data
        .join("pulsewire")
        .join(service)
        .join("config.toml")
    );
  }
  if let Some(home) = user_home {
    candidates.push(
      home
        .join(".config/pulsewire")
        .join(service)
        .join("config.toml")
    );
  }

  if let Some(bin_dir) =
    executable.and_then(Path::parent)
  {
    candidates.push(
      bin_dir
        .join("../share/pulsewire/config")
        .join(service)
        .join("config.toml")
    );
    candidates.push(
      bin_dir
        .join("../etc/pulsewire")
        .join(service)
        .join("config.toml")
    );
  }

  if let Some(cwd) = cwd {
    candidates.push(
      cwd
        .join("crates")
        .join(service)
        .join("res/config.toml")
    );
    candidates.push(
      cwd
        .join(service)
        .join("res/config.toml")
    );
    candidates.push(
      cwd.join("res/config.toml")
    );

    if service == "fetcher" {
      candidates.push(
        cwd.join(
          "src/main/resources/config/config.toml"
        )
      );
    }
  }

  if let Some(existing) =
    candidates.iter().find(|path| {
      path.is_file()
    })
  {
    return existing.clone();
  }

  // Give an installed binary a useful
  // missing-config destination; preserve
  // the historic development fallback
  // if HOME/XDG are unavailable.
  candidates
    .into_iter()
    .next()
    .unwrap_or_else(|| {
      PathBuf::from("crates")
        .join(service)
        .join("res/config.toml")
    })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn fixture_root(
    label: &str
  ) -> PathBuf {
    let root = std::env::temp_dir()
      .join(format!(
        "pw-config-{}-{label}",
        std::process::id()
      ));
    let _ =
      std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)
      .expect("fixture root");
    root
  }

  fn touch(path: &Path) {
    std::fs::create_dir_all(
      path.parent().expect("parent")
    )
    .expect("fixture directory");
    std::fs::write(path, "")
      .expect("fixture file");
  }

  #[test]
  fn cli_wins_over_environment_even_when_missing()
   {
    let root = fixture_root("cli");
    let cli = root.join("missing.toml");
    let env_path =
      root.join("environment.toml");
    touch(&env_path);
    assert_eq!(
      resolve_config_path(
        "fetcher",
        cli.to_str(),
        env_path.to_str(),
        None,
        None,
        None,
        Some(&root)
      ),
      cli
    );
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn environment_wins_over_xdg_and_checkout()
   {
    let root = fixture_root("env");
    let env_path =
      root.join("environment.toml");
    let xdg = root.join("xdg");
    let checkout =
      root.join("checkout");
    touch(&xdg.join(
      "pulsewire/server/config.toml"
    ));
    touch(&checkout.join(
      "crates/server/res/config.toml"
    ));
    assert_eq!(
      resolve_config_path(
        "server",
        None,
        env_path.to_str(),
        Some(&xdg),
        None,
        None,
        Some(&checkout)
      ),
      env_path
    );
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn xdg_and_home_precede_checkout()
   {
    let root = fixture_root("xdg");
    let xdg = root.join("xdg");
    let home = root.join("home");
    let checkout =
      root.join("checkout");
    let xdg_path = xdg.join(
      "pulsewire/fetcher/config.toml"
    );
    let home_path = home.join(
      ".config/pulsewire/fetcher/config.toml"
    );
    let workspace = checkout.join(
      "crates/fetcher/res/config.toml"
    );
    touch(&xdg_path);
    touch(&home_path);
    touch(&workspace);
    assert_eq!(
      resolve_config_path(
        "fetcher",
        None,
        None,
        Some(&xdg),
        Some(&home),
        None,
        Some(&checkout)
      ),
      xdg_path
    );
    std::fs::remove_file(&xdg_path)
      .expect("remove XDG fixture");
    assert_eq!(
      resolve_config_path(
        "fetcher",
        None,
        None,
        Some(&xdg),
        Some(&home),
        None,
        Some(&checkout)
      ),
      home_path
    );
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn installed_share_precedes_checkout()
   {
    let root = fixture_root("share");
    let bin = root.join("bin");
    let exe =
      bin.join("pulsewire-server");
    let share = bin.join(
      "../share/pulsewire/config/server/config.toml"
    );
    let checkout =
      root.join("checkout");
    touch(&share);
    touch(&checkout.join(
      "crates/server/res/config.toml"
    ));
    assert_eq!(
      resolve_config_path(
        "server",
        None,
        None,
        None,
        None,
        Some(&exe),
        Some(&checkout)
      ),
      share
    );
    let _ = std::fs::remove_dir_all(root);
  }

  #[test]
  fn checkout_fallback_works_without_install()
   {
    let root = fixture_root("dev");
    let expected = root.join(
      "crates/fetcher/res/config.toml"
    );
    touch(&expected);
    assert_eq!(
      resolve_config_path(
        "fetcher",
        None,
        None,
        None,
        None,
        None,
        Some(&root)
      ),
      expected
    );
    let _ = std::fs::remove_dir_all(root);
  }
}
