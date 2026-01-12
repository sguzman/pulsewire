# feedrv3

Rust async worker that polls a set of RSS/Atom feeds, tracks HTTP state with adaptive backoff, and stores feed payloads plus items in SQLite or Postgres. Configuration is file based (TOML) and a scheduler drives concurrent HEAD/GET requests with per-domain limits. The repo also ships a companion HTTP server for feed reader clients.

## Overview
- Fetcher loads app/domain/feed config from a TOML bundle, migrates/creates a SQL database (SQLite or Postgres) and bulk-ingests feed definitions.
- Scheduler ticks every 5s, finds due feeds, and processes them with bounded parallelism. Per-domain semaphores prevent hammering the same host; optional global cap controls total concurrency.
- Each feed alternates HEAD/GET based on last state. HEAD decides whether content changed; GET parses the body (via `feed-rs`), hashes it, and stores payload + items. Errors trigger exponential backoff with jitter and persisted state.
- Server provides auth, subscriptions, read/unread state, folders, favorites, and search APIs for clients. It reads from the fetcher database schema and maintains its own state in a separate schema.

## Code Layout
- `crates/core/src/` – shared runtime logic (config, scheduler, infra, ports, domain, feed parsing).
- `crates/fetcher/src/main.rs` – fetcher entrypoint; config loading, repo init/migrations, optional ingest benchmark, then scheduler loop.
- `crates/server/src/main.rs` – server entrypoint; config loading, schema apply, and HTTP routes.
- `crates/cli/src/main.rs` – ops CLI for validation/cleanup commands.
- `crates/*/README.md` – crate-specific docs (core/fetcher/cli).
- `crates/fetcher/res/` – example config bundle (`config.toml`, `domains.toml`, `feeds/*.toml`).
- `crates/server/res/` – server config and OpenAPI docs (`config.toml`, `openapi.json`, `openapi.html`).

## Configuration
Fetcher config resolution order:
1) CLI argument path (if provided), else
2) `CONFIG_PATH` environment variable (if set), else
3) `crates/fetcher/res/config.toml` when present.
Feed definitions default to `feeds/` under the config directory, but can be overridden with `FEEDS_DIR`.

`config.toml` (fetcher app sections):
- `[app]` – `mode` (`dev` deletes the DB on boot; `prod` leaves it intact) and `timezone`.
- `[database]` – `dialect` (`sqlite` default, or `postgres`).
- `[sqlite]` – `path` to the SQLite file.
- `[postgres]` – connection params: `user`, `password`, `host`, `port`, `database`, `ssl_mode`, `schema` (fetcher schema).
- `[polling]` – `default_seconds`, `max_seconds`, `jitter_fraction`.
- `[backoff]` – `error_base_seconds`, `max_error_seconds`.
- `[requests]` – `global_max_concurrent_requests` and `user_agent`.
- `[state_history]` – `sample_rate` between 0–1 for historical state rows.
- `[logging]` – `level`; `file_enabled`, `file_level`, `file_directory`, `file_rotation`.
- `[metrics]` – `enabled` toggles the Prometheus endpoint; `bind` sets the listen address.

`domains.toml`: list of `{ name, max_concurrent_requests }` entries limiting concurrent requests per host.

`feeds/*.toml`: one or more files shaped as `[[feeds]] { id, url, base_poll_seconds?, category?, provenance?, tags?, language?, content_type?, id_prefix? }`.
File-level defaults can be set at top-level (`base_poll_seconds`, `id_prefix`, `category`, `provenance`, `tags`, `language`, `content_type`) and are inherited by feeds that omit them.

Server config (`crates/server/res/config.toml`):
- `[app]` – `mode` and `timezone`.
- `[http]` – `host`, `port`.
- `[database]` – `dialect`.
- `[sqlite]` – `path`.
- `[postgres]` – connection params plus `schema` (server schema) and `fetcher_schema`.
- `[logging]` – `level`.
- `[auth]` – `token_ttl_seconds`.
- `[dev]` – `reset_on_start` (clears server-only tables).

## HTTP Server API (high level)
- Auth: login/logout, list/revoke tokens.
- Users: create user, change password.
- Feeds: list feeds, list feed entries.
- Entries: list, detail, read/unread, batch read/unread, unread counts, search.
- Subscriptions: list/create/delete.
- Folders: CRUD, assign/remove feeds, unread counts per folder.
- Favorites: list/add/remove.

OpenAPI docs:
- Spec: `GET /openapi.json`
- UI: `GET /docs`

## CLI Usage
- Run fetcher scheduler (default config resolution):
  `cargo run -p fetcher --release`
- Run fetcher with explicit config:
  `cargo run -p fetcher --release -- /path/to/config.toml`
- Ingest benchmark only (no scheduler):
  `cargo run -p fetcher --release -- --ingest-benchmark 50000`
- Validate config + semantic checks:
  `cargo run -p feedrv3-cli -- validate /path/to/config.toml`
- Clean local SQLite + logs (requires flag):
  `cargo run -p feedrv3-cli -- clean /path/to/config.toml --confirm`
- Run server (default config):
  `cargo run -p feedrv3-server --release`
- Run server with explicit config:
  `SERVER_CONFIG_PATH=/path/to/config.toml cargo run -p feedrv3-server --release`

## Data & Schema Notes
- Fetcher DDL lives in `crates/core/res/sql/{sqlite,postgres}/schema.sql`.
- Server DDL lives in `crates/server/res/sql/{sqlite,postgres}/schema.sql`.
- Postgres uses separate schemas: `fetcher` for fetcher tables, `server` for server state.

## Development
- Build: `cargo build`
- Tests: `cargo test`
- TOML validation: `taplo validate`
