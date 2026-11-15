# Pulsewire

<a href="#"><img src="https://raw.githubusercontent.com/catppuccin/catppuccin/main/assets/palette/macchiato.png" width="600px"/></a>

![Pulsewire banner](branding/pulsewire-banner.png)

<div align="center">
  <a href="https://github.com/sguzman/pulsewire/issues">
    <img src="https://img.shields.io/github/issues/sguzman/pulsewire?color=fab387&labelColor=303446&style=for-the-badge">
  </a>
  <a href="https://github.com/sguzman/pulsewire/stargazers">
    <img src="https://img.shields.io/github/stars/sguzman/pulsewire?color=1f6feb&labelColor=303446&style=for-the-badge">
  </a>
  <a href="https://github.com/sguzman/pulsewire">
    <img src="https://img.shields.io/github/repo-size/sguzman/pulsewire?color=ff4d4d&labelColor=303446&style=for-the-badge">
  </a>
  <a href="LICENSE">
    <img src="https://img.shields.io/static/v1.svg?style=for-the-badge&label=License&message=MIT&logoColor=ffffff&colorA=313244&colorB=2ea043"/>
  </a>
</div>

Pulsewire is a time-aware ingestion engine for the open web: RSS/Atom feeds today, release calendars and structured datasets next. It polls with adaptive backoff, respects per-domain limits, stores payloads + items in SQLite/Postgres, and ships a companion HTTP server for reader clients.

## Overview

- Fetcher loads app/domain/feed config from a TOML bundle, migrates/creates a
  SQL database (SQLite or Postgres) and bulk-ingests feed definitions.
- Scheduler ticks every 5s, finds due feeds, and processes them with bounded
  parallelism. Per-domain semaphores prevent hammering the same host; optional
  global cap controls total concurrency.
- Each feed alternates HEAD/GET based on last state. HEAD decides whether
  content changed; GET parses the body (via `feed-rs`), hashes it, and stores
  payload + items. Errors trigger exponential backoff with jitter and persisted
  state.
- Server provides auth, subscriptions, read/unread state, folders, favorites,
  and search APIs for clients. It reads from the fetcher database schema and
  maintains its own state in a separate schema.

## Code Layout

- `crates/core/src/` – shared runtime logic (config, scheduler, infra, ports,
  domain, feed parsing).
- `crates/fetcher/src/main.rs` – fetcher entrypoint; config loading, repo
  init/migrations, optional ingest benchmark, then scheduler loop.
- `crates/server/src/main.rs` – server entrypoint; config loading, schema apply,
  and HTTP routes.
- `crates/cli/src/main.rs` – ops CLI for validation/cleanup commands.
- `crates/tui/src/main.rs` – interactive terminal UI for the server API.
- `crates/*/README.md` – crate-specific docs (core/fetcher/cli).
- `crates/fetcher/res/` – example config bundle (`config.toml`, `domains.toml`,
  `feeds/*.toml`).
- `crates/server/res/` – server config and OpenAPI docs (`config.toml`,
  `openapi.json`, `openapi.html`).
- `branding/` – logo/banner/icon assets.

Login protection: the HTTP server persists atomic per-username and per-TCP-peer
attempt counters in its selected SQLite/PostgreSQL database. A username gets
five evaluated attempts per 15-minute window; the connected TCP peer gets
60 attempts across all usernames in the same window. Further attempts receive
HTTP 429 and `Retry-After: 900`. Successful login clears only the username
counter; source quotas continue to apply. Expired windows reset automatically.
Known and unknown usernames share generic invalid-credentials responses.
Caller-supplied `X-Forwarded-For` and similar headers are **never** trusted as
source identity. A reverse proxy or NAT can cause multiple users to share one
peer quota, and targeted attempts may temporarily block an account. Enforce
per-real-client limits at a separately trusted reverse proxy, restrict network
access, and follow the [deployment security runbook](docs/pulsewire/DEPLOYMENT.md)
before exposing the server.

## Configuration
Fetcher configuration lookup: 1) explicit positional config path, 2)
`CONFIG_PATH`, then 3) the shared user/installed/checkout lookup (existing
XDG configuration, HOME configuration, installed share paths, checkout
`crates/fetcher/res/config.toml`). The server uses `SERVER_CONFIG_PATH`
before its own shared fallback path. The ops CLI is different: it uses an
explicit command path, then `CONFIG_PATH`, then the checkout fetcher config.
An explicitly supplied invalid path fails rather than silently falling back.
Feed definitions default to `feeds/` under the selected config directory,
overridable with `FEEDS_DIR`. See
[compatibility and config lookup](docs/pulsewire/COMPATIBILITY.md).

`config.toml` (fetcher app sections):

- `[app]` – `mode` (`dev` and `prod` both preserve existing databases on boot) and
  `timezone`.
- `[database]` – `dialect` (`sqlite` default, or `postgres`).
- `[sqlite]` – `path` to the SQLite file.
- `[postgres]` – connection params: `user`, `password`, `host`, `port`,
  `database`, `ssl_mode`, `schema` (fetcher schema).
- `[polling]` – `default_seconds`, `max_seconds`, `jitter_fraction`.
- `[backoff]` – `error_base_seconds`, `max_error_seconds`.
- `[requests]` – `global_max_concurrent_requests` and `user_agent`.
- `[state_history]` – `sample_rate` between 0–1 for historical state rows.
- `[logging]` – `level`; `file_enabled`, `file_level`, `file_directory`,
  `file_rotation`.
- `[metrics]` – `enabled` toggles the Prometheus endpoint; `bind` sets the
  listen address.

`domains.toml`: list of `{ name, max_concurrent_requests }` entries limiting concurrent requests per host.

`feeds/*.toml`: one or more files shaped as
`[[feeds]] { id, url, base_poll_seconds?, category?, provenance?, tags?, language?, content_type?, id_prefix? }`.
File-level defaults can be set at top-level (`base_poll_seconds`, `id_prefix`,
`category`, `provenance`, `tags`, `language`, `content_type`) and are inherited
by feeds that omit them.

PostgreSQL credentials: checked-in examples have an empty password. The fetcher
and server support a private mounted file with
`PULSEWIRE_POSTGRES_PASSWORD_FILE` (recommended where possible), or a nonempty
`PULSEWIRE_POSTGRES_PASSWORD` environment variable, or private TOML
configuration. A file and direct env password cannot be set together.
Missing/invalid files fail closed. The default Compose examples allow empty
direct-password environment values so their optional file-secret overrides
can be parsed; **database startup still rejects an empty effective password**.
Never commit real credentials or treat a mount as a full secret manager.
See [file-backed secrets](docs/pulsewire/SECRETS.md).

Container deployment note: use an explicit existing Docker network shared with
PostgreSQL (`PULSEWIRE_DOCKER_NETWORK`) and optionally override its DNS hostname
(`PULSEWIRE_POSTGRES_HOST`, default `postgres`). The Compose examples do not
hard-code a DNS server or publish their ports to the LAN. See
[`docs/pulsewire/DEPLOYMENT.md`](docs/pulsewire/DEPLOYMENT.md) for a safe runbook.

Server config (`crates/server/res/config.toml`):

- `[app]` – `mode` and `timezone`.
- `[http]` – `host`, `port`.
- `[database]` – `dialect`.
- `[sqlite]` – `path`.
- `[postgres]` – connection params plus `schema` (server schema) and
  `fetcher_schema`.
- `[logging]` – `level`.
- `[auth]` – `token_ttl_seconds`.
- `[dev]` – `reset_on_start` is deprecated; enabling it is rejected to prevent data loss.
  Legacy `[seed]` values are ignored. Pulsewire never seeds `admin/admin` automatically.

## HTTP Server API (high Level)

- Auth: login/logout, rotate token, list/revoke tokens.
- Users: administrators create accounts; users can change their own password and
  delete their account. Self-service password
  reset is intentionally unavailable until a secure recovery flow is added.
- Feeds: list feeds, feed detail, list feed entries.
- Entries: list, detail, read/unread, batch read/unread, unread counts, search.
- Subscriptions: list/create/delete.
- Folders: CRUD, assign/remove feeds, list folder entries, unread counts (folder
  - per-feed).
- Favorites: list/add/remove feeds, unread counts.

Browser reader (PW-060, verified in isolated CI): `GET /reader` serves the embedded,
dependency-free responsive web app from the same Rust server. Log in using an
existing account; there is no self-registration or automatic administrator
account. The reader uses the existing /v1 APIs for entry lists and details,
global search, read/unread state, feed subscriptions, favorite **feeds**, and
folders. Source selection only supports feeds present in the server catalog.
Use / for search, j/k for next/previous entry, r to toggle read state, and
Escape to return from mobile article view. Appearance can be System (default),
Light, or Dark.

The browser keeps the bearer token in per-tab sessionStorage and clears it on
sign-out; appearance is the only localStorage preference. The reader does not
persist passwords, load third-party JavaScript, render untrusted feed markup as
HTML, or enable public network exposure. Treat any browser profile with an
active token as sensitive; deploy only after the documented PW-023 ingress/TLS
and network-security acceptance checks. No npm/build tooling is needed.

API pagination and count semantics (PW-061, verified in isolated CI):

- Entry listing/search (including feed and folder entries) is ordered by
  **descending ingestion item ID**, not publication date. `limit` defaults to
  50 and is clamped to 1–200; `offset` starts at zero.
- Responses include `items`, `next_offset`, `next_cursor` and `since`.
  Use `next_offset` to request the next older page; it is `null` when no
  further results exist. `next_cursor` is the **highest returned item ID on
  this page** (not an older-page token); `since=N` filters to IDs greater
  than N, enabling polling for newly ingested entries. Offset paging does not
  promise a fixed snapshot if new items arrive during navigation.
- Search `q` is a trimmed, case-insensitive **literal substring** across
  title, summary and description. SQL wildcard characters are not operators.
  `read` accepts `all`, `read` or `unread`; invalid values return HTTP
  400.
- `/v1/entries/unread/count` and `/v1/feeds/unread/counts` count the
  authenticated user's **subscribed feeds**. `/v1/feeds/counts` covers the
  complete catalog (including feeds with zero entries), with personalized
  per-user read/unread state.
- Source-linked cross-backend verification:
  [PW061 acceptance](docs/pulsewire/PW061-ACCEPTANCE.md). The API does not
  expose the disabled public password-reset workflow.

OpenAPI docs:

- Spec: `GET /openapi.json`
- UI: `GET /docs`

## CLI Usage

- Run fetcher scheduler (default config resolution):
  `cargo run -p pulsewire-fetcher --release`
- Run fetcher with explicit config:
  `cargo run -p pulsewire-fetcher --release -- /path/to/config.toml`
- Ingest benchmark only (no scheduler):
  `cargo run -p pulsewire-fetcher --release -- --ingest-benchmark 50000`
- Validate config + semantic checks:
  `cargo run -p pulsewire-cli -- validate /path/to/config.toml`
- Clean *isolated* SQLite development files only (**destructive; do not run on an existing home database**): the configuration must have
  `app.mode = "dev"` and `database.dialect = "sqlite"`, both targets must be
  inside an existing dedicated `PULSEWIRE_DEV_DATA_DIR`, and the command requires
  `--confirm` plus `PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET=YES`. Production data
  cannot be cleaned through this command. No service startup performs cleanup.
- Explicit offline administrator bootstrap (requires at least a 12-byte password;
  with an existing database, authenticate an existing account for promotion):
  `PULSEWIRE_BOOTSTRAP_USERNAME=<name> PULSEWIRE_BOOTSTRAP_PASSWORD=<passphrase> cargo run -p pulsewire-server --release -- --bootstrap-admin`
  The bootstrap command does not open an HTTP listener. Remove bootstrap credentials
  from the environment afterward. `POST /v1/users` requires an admin bearer token.
- Offline password recovery for a database operator (no public reset endpoint):
  `PULSEWIRE_RECOVERY_CONFIRM=RECOVER_USER PULSEWIRE_RECOVERY_USERNAME=<existing-user> PULSEWIRE_RECOVERY_PASSWORD=<new-passphrase> cargo run -p pulsewire-server --release -- --recover-user`
  Requires the database configuration/credentials, a password of at least 12 bytes,
  and an explicit confirmation. Revokes all previous sessions/reset tokens without
  changing the user's role. Keep environment secrets private and clear them afterward.
- Run server (default config): `cargo run -p pulsewire-server --release`
- Run server with explicit config:
  `SERVER_CONFIG_PATH=/path/to/config.toml cargo run -p pulsewire-server --release`
- Run TUI (uses the TUI config for server URL):
  `PULSEWIRE_TUI_CONFIG=crates/tui/res/config.toml cargo run -p pulsewire-tui`

## Data & Schema Notes

- Fetcher DDL lives in `crates/core/res/sql/{sqlite,postgres}/schema.sql`.
- Server DDL lives in `crates/server/res/sql/{sqlite,postgres}/schema.sql`.
- Postgres uses separate schemas: `fetcher` for fetcher tables, `server` for
  server state.

## Development

- Tooling guidance: [`docs/reference/ai/tools/ADDING.md`](docs/reference/ai/tools/ADDING.md)
- Tooling: cargo-release, git-cliff, just, biome, taplo, rustfmt, lychee, typos
- Justfile: `just build`, `just fmt`, `just validate`, `just test`,
  `just all`
- Release policy: [`docs/reference/RELEASE.md`](docs/reference/RELEASE.md)
- Roadmap: [`docs/pulsewire/ROADMAP.md`](docs/pulsewire/ROADMAP.md)
- Operator docs: [deployment preflight](docs/pulsewire/DEPLOYMENT.md),
  [compatibility/defaults/privacy](docs/pulsewire/COMPATIBILITY.md),
  [troubleshooting](docs/pulsewire/TROUBLESHOOTING.md),
  [backup/restore](docs/pulsewire/BACKUP-RESTORE.md),
  [operator recovery](docs/pulsewire/RECOVERY.md),
  [network fetch safety](docs/pulsewire/FETCH_SECURITY.md), and
  [monitoring/readiness](docs/pulsewire/MONITORING.md),
  [file-backed database secrets](docs/pulsewire/SECRETS.md),
  [optional TLS ingress and network isolation](docs/pulsewire/INGRESS.md),
  [authenticated metrics](docs/pulsewire/METRICS-SECURITY.md),
  and [read-only host exposure preflight](docs/pulsewire/HOST-PREFLIGHT.md).
  [PW-070 documentation acceptance](docs/pulsewire/PW070-ACCEPTANCE.md)
  is verified in isolated CI. All operator procedures are **preflight or
  isolated CI reference**, not a deployment or release authorization.
- Changelog config: `cliff.toml`
- Release config: `release.toml`
- License inventory:
  `cargo about generate about.hbs > docs/THIRD_PARTY_LICENSES.md`
- Build: `cargo build`
- Tests: `cargo test`
- TOML validation: `taplo validate`
- Link validation: `lychee --config lychee.toml .`
- JSON formatting: `biome format --write .`

## License
MIT.
