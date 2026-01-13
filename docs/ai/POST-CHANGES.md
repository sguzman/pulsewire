# Post-Change Checklist (Manual)

Run these commands after a change is completed and confirmed to compile.

## Format Rust

- `cargo fmt`

## Format TOML

- `taplo fmt`

## Validate TOML

- `taplo validate`

## Format JSON (Biome)

- `biome format --write <files-or-directories>`
  - Example: `biome format --write path/to/file.json`
  - Example: `biome format --write .`

## Tests

- `cargo test`

## Docs

- Update any docs that changed behavior/config/API
