# Pulsewire CLI (pulsewire-Cli)

Operations CLI for the fetcher config bundle. It validates schema + semantic rules and can clean local dev artifacts with a safety flag.

## Commands

- `validate [config_path]` – validate TOML schemas and semantic rules.
- `clean [config_path] --confirm` – remove SQLite DB and log directory for dev
  cleanup.

## Config Resolution
If no path is provided, the CLI uses: 1) `CONFIG_PATH` environment variable if
set 2) `crates/fetcher/res/config.toml`

## Examples

- Validate default config: `cargo run -p pulsewire-cli -- validate`
- Validate explicit config:
  `cargo run -p pulsewire-cli -- validate /path/to/config.toml`
- Destructive cleanup is **never a normal setup or troubleshooting step**.
  This command requires **all** of: `app.mode = "dev"`, a SQLite
  `database.dialect`, a current `--confirm` argument,
  `PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET=YES`, and an existing dedicated,
  nested `PULSEWIRE_DEV_DATA_DIR` containing **both** realpaths of the
  target SQLite file and log directory. The CLI rejects a symlink outside
  the root, non-SQLite production DBs and missing confirmation. Never aim
  it at a real host, a shared database or a production data directory.

## Notes

- `clean` is **destructive** and requires all isolation and explicit
  opt-in guards described above, not just `--confirm`.
- Validation is non-destructive, but should still use a private or synthetic
  config rather than printing sensitive input.
- Validation uses the JSON schemas under `crates/schemas/fetcher/`.
