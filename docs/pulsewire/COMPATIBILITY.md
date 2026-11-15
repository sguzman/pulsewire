# Pulsewire — compatibility and configuration contract

**Status (2026-11-15):** Documented from source and isolated CI. This is
not a home-host deployment approval or a promise that all hosts, database
versions, browser engines, or existing datasets have been exercised. For
deployment preflight see [DEPLOYMENT.md](DEPLOYMENT.md).

## Components and verified environments

| Component | Source-backed behavior | Verification boundary |
|---|---|---|
| Fetcher | Rust binary named `pulsewire` from package `pulsewire-fetcher`; polls configured RSS/Atom feeds and enabled HTML watches, writing item and history tables | Rust tests and disposable non-root container smoke; real feed compatibility and host resource envelope are not accepted |
| Reader API | Rust binary and package `pulsewire-server`; authenticated `/v1` API, `/health`, `/ready`, `/reader` | Isolated PostgreSQL and SQLite HTTP acceptance (PW-051/061); ingress/TLS on a household network not accepted |
| Terminal client | `pulsewire-tui` uses the authenticated HTTP API | Scripted App/key-handler journeys on disposable PostgreSQL **and** SQLite, not human terminal rendering on every emulator |
| Web reader | Embedded HTML/CSS/JavaScript from the Rust server; no npm and no third-party runtime scripts | Chromium CI at 1280px and 440px including real PostgreSQL API; other browser engines/devices need operator QA |
| PostgreSQL | Separate configured `fetcher` and `server` schemas in the **same selected database**; server needs existing fetcher tables | Disposable PostgreSQL 16 container and backup/restart tests; no claim for all PostgreSQL versions or a pre-existing household schema |
| SQLite | The fetcher and server must point at the **same compatible database file** for feed items, users and read state; server applies its schema | Disposable independent SQLite file, migration/restart/backup and TUI/API tests; arbitrary split-file installations are not established |
| Container target | `x86_64-unknown-linux-musl` default in Dockerfiles; `rust:1.91-alpine` builder, `scratch` final stage, UID/GID 65532 | CI checks non-root image packaging and memory/CPU/PID limits; architecture-specific performance on the home PC is not measured |
| Local development | Workspace uses `rust-toolchain.toml` with nightly plus `rustfmt`/`clippy`; crates use edition 2024 | GitHub Actions format/compile/tests/Clippy. A Docker builder version is **not** a promise that arbitrary local stable toolchains suffice |

Reference CI evidence: [PW-051](PW051-ACCEPTANCE.md),
[PW-061](PW061-ACCEPTANCE.md), and [PW-043](PW043-ACCEPTANCE.md).
Remaining deployment checks live in [ROADMAP.md](ROADMAP.md).

## Configuration discovery

The fetcher prioritizes an **explicit positional path**, then
`CONFIG_PATH`. The server prioritizes `SERVER_CONFIG_PATH`. The shared
runtime fallback checks existing XDG user configuration, HOME configuration,
executable-relative installed configuration, then checkout defaults; an
explicit invalid path fails instead of silently selecting another file.
See `crates/schemas/src/config_paths.rs` for the full precedence.

The ops CLI is different: its optional command path takes precedence over
`CONFIG_PATH`, otherwise it uses `crates/fetcher/res/config.toml`. Do not
assume CLI and service fallback chains are identical.

`SCHEMAS_DIR` overrides runtime schema discovery. Installed services can
use `$XDG_DATA_HOME/pulsewire/schemas/{fetcher,server}`, a HOME share
fallback, executable-adjacent `share/pulsewire/schemas`, or local checkout
schema trees. The supplied container configurations set
`SCHEMAS_DIR=/app/schemas/{fetcher,server}`; configuration and validation
schemas must belong to the same deployed version. An invalid explicit
override is an error, not permission to bypass validation.

Default checkout configs use PostgreSQL with an **empty password**.
`PULSEWIRE_POSTGRES_PASSWORD` overrides nonempty private PostgreSQL
passwords; `PULSEWIRE_POSTGRES_HOST` overrides the database hostname. Do
not paste either credential into checked-in TOML, commits or support logs.

## Default Docker network, exposure and feed scope

- Compose requires an **existing external** network named by
  `PULSEWIRE_DOCKER_NETWORK`; it does not create or select a household
  network. The default database DNS alias is `postgres` unless explicitly
  overridden. The default Docker credentials/name/schemas in the examples
  are `admin`, `data`, `fetcher`, and `server`: **examples**, not a
  confirmed match for any host. Optional
  [Compose file-secret overlays](SECRETS.md) remove password **bytes** from
  container environment values, not from the access of Docker administrators.
- Server config binds `0.0.0.0:8091` **inside the container**, while the
  Compose host publishing is `127.0.0.1:8091:8091` only. Containers on the
  selected network may still reach the internal server listener. There is
  no configured HTTPS proxy or public ingress.
- Fetcher Prometheus metrics are **disabled by default**. If enabled without
  replacing the example bind, the endpoint uses `127.0.0.1:9898`
  **inside the container**; Compose does not publish it. A separately reviewed
  private metrics deployment is required.
- YouTube and podcasts are **disabled and preserved**. Docker's
  `.dockerignore` excludes their large trees from the default image;
  `categories.toml` disables their scheduling even with mounted feed trees.
  Removing definitions or historical rows is **not** part of this contract.
  HTML watches with explicit `enabled = false` are suppressed by the
  persistent due-queue state; deleting a declaration alone is **not** an
  accepted replacement for explicit disabling.
- Outbound fetches enforce conservative URL, redirect, DNS and body-size
  policies. `PULSEWIRE_ALLOW_PRIVATE_FEEDS=1` is only for reviewed,
  trusted internal sources, and is used in CI to reach its private RSS
  fixture; it is **not** the standard deployment default.

Review [FETCH_SECURITY.md](FETCH_SECURITY.md) and
[MONITORING.md](MONITORING.md) before changing networking.

## Privacy and client-side state

Accounts, password hashes, bearer-token hashes, read state, folder/favorite
membership, feed URLs and ingested content reside in the configured database.
Database backups can therefore contain private records and must be handled
as sensitive data. The browser retains its bearer token in **per-tab
`sessionStorage`** until sign-out/tab-session end and stores only the
appearance preference in `localStorage`; it renders feed text without
interpreting HTML, loads no third-party JavaScript, and only opens
HTTP(S) article links. Tokens can still be stolen from an already
compromised browser profile or exposed by unsafe HTTP on an untrusted
network. The TUI's checked-in config leaves `auto_login = false` and
`password = ""`; do not populate a tracked file with real credentials.
Optional user-provided cookie/header files used for fetching are also
sensitive; Compose mounts their directories read-only.

The server deliberately ignores untrusted `X-Forwarded-For` headers when
calculating login quotas. A trusted reverse proxy must separately enforce
a per-real-client policy if ingress is ever approved. This and TLS/secrets
are **unverified PW-023 work**, not solved by changing the local HTTP bind.

## Compatibility-changing operations

Do not upgrade or roll back a host by simply pointing an older binary at a
newer migrated database. Rehearse **both** the migration and recovery on a
fresh restored copy, with fetcher and server schema/config versions matched.
The current schema acceptance is idempotency and preservation in CI; generic
downgrade compatibility and real-data migration speed are **not** proven.
See [BACKUP-RESTORE.md](BACKUP-RESTORE.md) and
[RECOVERY.md](RECOVERY.md).

`app.mode = "dev"` and `"prod"` both **preserve data on startup**;
server `dev.reset_on_start = true` is explicitly rejected. The CLI's
deliberately destructive `clean` command is restricted to a separately
designated, nested SQLite dev-data directory with `--confirm`,
`PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET=YES`, and
`PULSEWIRE_DEV_DATA_DIR`; it does not clean production databases.
Unauthenticated password resets remain disabled; offline administrator
bootstrap and offline password recovery require a trusted database
operator and explicit private credentials. No release tag or household
deployment has been approved.
