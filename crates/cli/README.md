# Pulsewire CLI (pulsewire-cli)

Operations CLI for the fetcher config bundle. It validates schema + semantic rules and can clean local dev artifacts with a safety flag.

## Commands
- `validate [config_path]` – validate TOML schemas and semantic rules.
- `clean [config_path] --confirm` – remove SQLite DB and log directory for dev cleanup.

## Config resolution
If no path is provided, the CLI uses:
1) `CONFIG_PATH` environment variable if set
2) `crates/fetcher/res/config.toml`

## Examples
- Validate default config:
  `cargo run -p pulsewire-cli -- validate`
- Validate explicit config:
  `cargo run -p pulsewire-cli -- validate /path/to/config.toml`
- Clean dev artifacts:
  `cargo run -p pulsewire-cli -- clean /path/to/config.toml --confirm`

## Notes
- `clean` is destructive and requires `--confirm`.
- Validation uses the JSON schemas under `crates/fetcher/res/schemas/`.
