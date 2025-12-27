# feedrv3

Rust async worker that polls a set of RSS/Atom feeds, tracks HTTP state with adaptive backoff, and stores feed payloads plus items in SQLite. Configuration is file based (TOML) and a tiny scheduler drives concurrent HEAD/GET requests with per-domain limits.

## Overview
- Loads app/domain/feed config from a TOML bundle, migrates/creates a SQL database (SQLite by default) and bulk-ingests feed definitions.
- Scheduler ticks every 5s, finds due feeds, and processes them with bounded parallelism. Per-domain semaphores prevent hammering the same host; optional global cap controls total concurrency.
- Each feed alternates HEAD/GET based on last state. HEAD decides whether content changed; GET parses the body (via `feed-rs`), hashes it, and stores payload + items. Errors trigger exponential backoff with jitter and persisted state.
- Minimal traits (`Repo`, `Http`, `Clock`, `RandomSource`) keep the core logic isolated; `SqliteRepo`, `ReqwestHttp`, `SystemClock`, and `MutexRng` are the shipped impls.
- Logging uses `tracing` with env-filter override support. Dev mode wipes the DB on startup for a clean slate.

## Code Layout
- `src/main.rs` – entrypoint; argument parsing, config loading, repo init/migrations, optional ingest benchmark, then scheduler loop.
- `src/app/` – `AppContext` wiring and `Scheduler` tick loop (due-feed selection, concurrency guards, fetch pipeline).
- `src/domain/` – core types: configs, link state machine, delay/backoff math, hashing helpers.
- `src/feed/` – feed parsing to normalized metadata/items.
- `src/infra/` – adapters: config loader, logging, random, reqwest HTTP client, SQLite repo, clock, time formatting.
- `src/ports/` – traits for HTTP, repo, clock, randomness.
- `tests/` – link state property-style tests.
- `res/` – example config bundle (`config.toml`, `domains.toml`, `feeds/*.toml`) and a sample SQLite DB snapshot.

## Configuration
Config is resolved from:
1) CLI argument path (if provided), else
2) `CONFIG_PATH` environment variable (if set), else
3) `res/config.toml` when present, else
4) `src/main/resources/config/config.toml` (legacy layout).
Feed definitions default to `feeds/` under the config directory, but can be overridden with `FEEDS_DIR`.

`config.toml` (app-wide sections):
- `[app]` – `mode` (`dev` deletes the DB on boot; `prod` leaves it intact) and `timezone` (IANA TZ for timestamps/logging).
- `[database]` – `dialect` (`sqlite` default, or `postgres`).
- `[sqlite]` – `path` to the SQLite file (relative paths resolve from the config dir unless the path includes `resources`, in which case CWD is used).
- `[postgres]` – connection params: `user`, `password`, `host`, `port`, `db` (defaults: admin/admin/localhost/5432/data).
- `[polling]` – `default_seconds`, `max_seconds`, and `jitter_fraction` controlling poll cadence and jitter.
- `[backoff]` – `error_base_seconds` and `max_error_seconds` bounding exponential backoff after errors.
- `[requests]` – `global_max_concurrent_requests` optional cap on in-flight HTTP requests (defaults to 64 when unset) and `user_agent` string.
- `[state_history]` – `sample_rate` between 0–1 for persisting historical state rows (current state is always stored).
- `[logging]` – `level` base log level (can be overridden by `RUST_LOG`).

`domains.toml`: list of `{ name, max_concurrent_requests }` entries limiting concurrent requests per host. Domains not listed default to a limit of 1.

`feeds/*.toml`: one or more files shaped as `[[feeds]] { id, url, base_poll_seconds? }`. Domain is derived from the URL host automatically; `base_poll_seconds` falls back to `polling.default_seconds` when omitted.

## Runtime & Orchestration
- DB migrations run on startup (tables for feeds, current+historical state, fetch events, payloads, and items). WAL is enabled.
- Scheduler:
  - Tick interval: 5s.
  - Due query: up to 1000 feeds whose `next_action_at_ms` has passed or are new.
  - Parallelism: `global_max_concurrent_requests` or default 64, with per-domain semaphores.
  - Actions: decide `SleepUntil`, `DoHead`, or `DoGet` from the persisted `LinkState`.
  - Backoff: exponential with jitter; errors increase backoff, unchanged HEADs move to sleep, body changes reset backoff.
- Events: every HEAD/GET is recorded in `fetch_events`; state snapshots go to `feed_state_current` (and optionally `feed_state_history`); payloads plus items are stored when GET bodies parse successfully.

## CLI Usage
- Run scheduler (using default config resolution):  
  `cargo run --release`
- Run scheduler with explicit config:  
  `cargo run --release -- /path/to/config.toml`
- Ingest benchmark only (no scheduler):  
  `cargo run --release -- --ingest-benchmark 50000`  
  Inserts synthetic feeds into the DB in bulk and exits. Requires a feed count > 0.

## Data & Schema Notes
- SQLite path comes from `[sqlite].path`; WAL mode and `synchronous` toggling are used to speed bulk upserts.
- Creation DDLs live in `res/sql/sqlite/schema.sql` and `res/sql/postgres/schema.sql` and are applied at startup; non-schema migrations remain in code.
- Postgres stores timestamps as `timestamptz`, using the timezone from config when converting epoch milliseconds to database values.
- Key tables: `feeds` (definitions), `feed_state_current` + `feed_state_history`, `fetch_events`, `feed_payloads`, `feed_items`.
- A prebuilt DB snapshot is checked in under `res/` for quick inspection; dev mode will delete it on boot.

## Development
- Build/test: `cargo test` (no extra setup needed; uses the traits to avoid network access in tests).
- Logs: configure via `logging.level` or `RUST_LOG`; log output includes targets and thread info.
- HTTP client: reqwest with rustls, 30s timeout, gzip/brotli/deflate enabled.

## Docker
- Image expects a full config bundle (config/domains/categories/feeds) to be present on disk.
- Default path inside the container is `/app/res/config.toml`; override with `CONFIG_PATH`.
- Feed definitions default to `/app/res/feeds`; override with `FEEDS_DIR`.
- For environment-specific settings, mount a config directory and point `CONFIG_PATH` at it.
