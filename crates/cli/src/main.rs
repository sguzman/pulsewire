use std::path::PathBuf;

use clap::{
  Parser,
  Subcommand
};
use feedrv3_core::infra::config::{
  ConfigLoader,
  LoadedConfig
};

#[derive(Parser)]
#[command(
  author,
  version,
  about = "feedrv3 ops CLI"
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

      feedrv3_core::infra::config::validate_semantic(&app, &categories)
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

      if matches!(app.db_dialect, feedrv3_core::domain::model::SqlDialect::Sqlite) {
                if let Err(e) = std::fs::remove_file(&app.sqlite_path) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        return Err(format!("failed to remove sqlite db: {e}"));
                    }
                }
            }

      if let Err(e) =
        std::fs::remove_dir_all(
          &app.log_file_directory
        )
      {
        if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!("failed to remove log directory: {e}"));
                }
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
  {
    if !p.trim().is_empty() {
      return PathBuf::from(p);
    }
  }

  PathBuf::from(
    "crates/fetcher/res/config.toml"
  )
}
