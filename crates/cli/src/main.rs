use std::path::{
  Path,
  PathBuf
};

use clap::{
  Parser,
  Subcommand
};
use pulsewire_core::domain::model::{
  AppMode,
  SqlDialect
};
use pulsewire_core::infra::config::{
  ConfigLoader,
  LoadedConfig
};

#[derive(Parser)]
#[command(
  author,
  version,
  about = "pulsewire ops CLI"
)]
struct Args {
  #[command(subcommand)]
  command: Command
}

#[derive(Subcommand)]
enum Command {
  /// Validate TOML config (schema +
  /// semantic checks).
  Validate {
    /// Path to config.toml (defaults
    /// to CONFIG_PATH or
    /// crates/fetcher/res/config.
    /// toml).
    config_path: Option<PathBuf>
  },
  /// Clean local dev artifacts (SQLite
  /// + logs) with a safety flag.
  Clean {
    /// Path to config.toml (defaults
    /// to CONFIG_PATH or
    /// crates/fetcher/res/config.
    /// toml).
    config_path: Option<PathBuf>,
    /// Required to perform destructive
    /// actions.
    #[arg(long)]
    confirm:     bool
  }
}

#[tokio::main]
async fn main() -> Result<(), String> {
  let args = Args::parse();

  match args.command {
    | Command::Validate {
      config_path
    } => {
      let cfg_path =
        pick_config_path(config_path);

      let LoadedConfig {
        app,
        categories,
        ..
      } = ConfigLoader::load(&cfg_path)
        .await
        .map_err(|e| e.to_string())?;

      pulsewire_core::infra::config::validate_semantic(&app, &categories)
                .map_err(|e| e.to_string())?;

      println!(
        "ok: config validated at {}",
        cfg_path.display()
      );
    }
    | Command::Clean {
      config_path,
      confirm
    } => {
      if !confirm {
        return Err(
          "refusing to clean without \
           --confirm"
            .to_string()
        );
      }

      let cfg_path =
        pick_config_path(config_path);

      let LoadedConfig {
        app, ..
      } = ConfigLoader::load(&cfg_path)
        .await
        .map_err(|e| e.to_string())?;

      if !matches!(
        app.mode,
        AppMode::Dev
      ) {
        return Err(
          "refusing cleanup: app.mode \
           must be dev"
            .into()
        );
      }
      if !matches!(
        app.db_dialect,
        SqlDialect::Sqlite
      ) {
        return Err(
          "refusing cleanup: only \
           isolated SQLite \
           development files are \
           supported"
            .into()
        );
      }
      if std::env::var("PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET").ok().as_deref() != Some("YES") {
        return Err("refusing cleanup: PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET=YES required".into());
      }
      let root = std::env::var(
        "PULSEWIRE_DEV_DATA_DIR"
      )
      .map_err(|_| {
        "PULSEWIRE_DEV_DATA_DIR is \
         required to bound cleanup"
          .to_string()
      })?;
      let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|e| {
          format!(
            "invalid development \
             cleanup directory: {e}"
          )
        })?;
      if !root.is_dir()
        || root.components().count() < 3
      {
        return Err(
          "refusing cleanup: \
           development root must be a \
           dedicated nested directory"
            .into()
        );
      }

      // Validate both targets before
      // deleting either one.
      let remove_db =
        checked_clean_target(
          &root,
          &app.sqlite_path,
          false
        )?;
      let remove_logs =
        checked_clean_target(
          &root,
          &app.log_file_directory,
          true
        )?;
      if remove_db {
        std::fs::remove_file(
          &app.sqlite_path
        )
        .map_err(|e| {
          format!(
            "failed to remove sqlite \
             db: {e}"
          )
        })?;
      }
      if remove_logs {
        std::fs::remove_dir_all(
          &app.log_file_directory
        )
        .map_err(|e| {
          format!(
            "failed to remove log \
             directory: {e}"
          )
        })?;
      }

      println!(
        "ok: cleaned local artifacts"
      );
    }
  }

  Ok(())
}

fn pick_config_path(
  arg: Option<PathBuf>
) -> PathBuf {
  if let Some(p) = arg {
    return p;
  }

  // CLI flags win; fall back to
  // CONFIG_PATH, then repo-local
  // defaults.
  if let Ok(p) =
    std::env::var("CONFIG_PATH")
    && !p.trim().is_empty()
  {
    return PathBuf::from(p);
  }

  PathBuf::from(
    "crates/fetcher/res/config.toml"
  )
}

fn checked_clean_target(
  root: &Path,
  target: &Path,
  directory: bool
) -> Result<bool, String> {
  // Nonexistent files are no-ops; never
  // resolve an absent target through an
  // unchecked path before deleting
  // anything.
  if !target.exists() {
    return Ok(false);
  }
  let actual = target
    .canonicalize()
    .map_err(|e| {
      format!(
        "cannot resolve cleanup \
         target: {e}"
      )
    })?;
  if actual == root
    || !actual.starts_with(root)
  {
    return Err(format!(
      "refusing cleanup outside \
       designated development \
       directory: {}",
      target.display()
    ));
  }
  if actual.is_dir() != directory {
    return Err(
      "cleanup target has unexpected \
       file type"
        .into()
    );
  }
  Ok(true)
}

#[cfg(test)]
mod cleanup_tests {
  use super::*;

  #[test]
  fn deletion_guard_refuses_outside_paths_and_root()
   {
    let root = std::env::temp_dir()
      .join(format!(
        "pulsewire-clean-test-{}",
        std::process::id()
      ));
    std::fs::create_dir_all(&root)
      .expect("dev root");
    let root = root
      .canonicalize()
      .expect("canonical root");
    let inside = root.join("db.sqlite");
    std::fs::write(
      &inside,
      b"synthetic test data"
    )
    .expect("db fixture");
    let outside = root
      .parent()
      .expect("parent")
      .join(format!(
        "pulsewire-outside-test-{}",
        std::process::id()
      ));
    std::fs::write(
      &outside, b"keep me"
    )
    .expect("outside fixture");
    assert!(
      checked_clean_target(
        &root, &inside, false
      )
      .expect("inside allowed")
    );
    assert!(
      checked_clean_target(
        &root, &outside, false
      )
      .is_err()
    );
    assert!(
      checked_clean_target(
        &root, &root, true
      )
      .is_err()
    );
    assert!(
      !checked_clean_target(
        &root,
        &root.join("missing"),
        false
      )
      .expect("missing no-op")
    );
    assert_eq!(
      std::fs::read(&outside)
        .expect("outside survives"),
      b"keep me"
    );
    std::fs::remove_file(outside)
      .expect("remove outside fixture");
    std::fs::remove_dir_all(root)
      .expect("remove dev fixture");
  }

  #[cfg(unix)]
  #[test]
  fn deletion_guard_rejects_symlinks_to_outside_file()
   {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir()
      .join(format!(
        "pulsewire-link-test-{}",
        std::process::id()
      ));
    std::fs::create_dir_all(&root)
      .expect("dev root");
    let root = root
      .canonicalize()
      .expect("canonical root");
    let outside = root
      .parent()
      .expect("parent")
      .join(format!(
        "pulsewire-link-target-{}",
        std::process::id()
      ));
    std::fs::write(
      &outside,
      b"do not remove"
    )
    .expect("outside");
    symlink(
      &outside,
      root.join("redirect.sqlite")
    )
    .expect("link");
    assert!(
      checked_clean_target(
        &root,
        &root.join("redirect.sqlite"),
        false
      )
      .is_err()
    );
    assert_eq!(
      std::fs::read(&outside)
        .expect("outside survives"),
      b"do not remove"
    );
    std::fs::remove_file(outside)
      .expect("cleanup outside");
    std::fs::remove_dir_all(root)
      .expect("cleanup root");
  }
}
