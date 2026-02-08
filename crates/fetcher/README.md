# Fetcher

Feed polling daemon that ingests RSS/Atom feeds into SQLite or Postgres. It uses the core scheduler and persists payloads/items for downstream consumers.

## Features

- Periodic scheduler with per-domain concurrency limits.
- HEAD/GET flow with adaptive backoff and jitter.
- Stores payloads, items, and fetch events.
- Dev mode can wipe DB on startup.

## Running

- Default config resolution: `cargo run -p pulsewire-fetcher --release`
- Explicit config:
  `cargo run -p pulsewire-fetcher --release -- /path/to/config.toml`
- Ingest benchmark (no scheduler):
  `cargo run -p pulsewire-fetcher --release -- --ingest-benchmark 50000`

## Config Files
Located under `crates/fetcher/res/` by default:

- `config.toml`
- `domains.toml`
- `categories.toml`
- `feeds/**/*.toml`

Feed files support global defaults in each file and per-feed overrides. See the root `README.md` for the full property list.

## Environment

- `CONFIG_PATH` – overrides config location.
- `FEEDS_DIR` – overrides feeds directory for the config bundle.

## Database

- SQLite and Postgres supported.
- DDL is under `crates/core/res/sql/`.
- Postgres uses the schema configured in `[postgres].schema`.
