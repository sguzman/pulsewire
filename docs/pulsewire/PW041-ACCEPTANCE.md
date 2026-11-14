# PW-041 — Portable runtime schema and configuration paths

**Status: verified complete, 2026-11-14.** Acceptance scope: M4 / PW-041,
three weighted points. This document describes an *installed-layout* contract,
not authorization to deploy or expose Pulsewire to a network.

## Runtime configuration precedence

The fetcher selects the first applicable location in this order:

1. Explicit positional CLI config path.
2. Nonempty `CONFIG_PATH`.
3. Existing `$XDG_CONFIG_HOME/pulsewire/fetcher/config.toml`.
4. Existing `$HOME/.config/pulsewire/fetcher/config.toml`.
5. Executable-adjacent installed share/config paths.
6. Checkout/development fallback (including old resource path).

The server uses nonempty `SERVER_CONFIG_PATH`, then the corresponding XDG,
HOME, executable-adjacent and development paths (no positional server config).
Explicitly provided paths are authoritative even when missing: a typo fails
rather than silently loading a different configuration.

## Validation schemas and migration SQL

The shared schema resolver selects a nonempty explicit `SCHEMAS_DIR`, otherwise
an installed XDG data schema location (or HOME default), executable-adjacent
`share/pulsewire/schemas/<service>`, then a checkout/developer path.

For an installed layout, provision schema JSON under
`share/pulsewire/schemas/fetcher` and `share/pulsewire/schemas/server`.
The **server also requires SQL migration files** under the installed schema
root at `server/sql/postgres/schema.sql` and
`server/sql/sqlite/schema.sql`, sourced from `crates/server/res/sql`.
For legacy deployments with `sql/<dialect>/schema.sql` beside the server
config, the config-adjacent file remains authoritative.

For user config, typical targets are:

- `$XDG_CONFIG_HOME/pulsewire/fetcher/config.toml`
- `$XDG_CONFIG_HOME/pulsewire/server/config.toml`

An executable-only `cargo install` does **not** install these data files:
the operator/packager must provision configuration and schema assets.
Docker's existing explicit paths are preserved, and its normal Compose
config remains unchanged.

## Verified evidence

- [PR #26](https://github.com/sguzman/pulsewire/pull/26), merged
  `7c651e1`: runtime validation schema resolution, XDG and executable-share
  precedence tests, full Rust and disposable container CI green.
- [PR #28](https://github.com/sguzman/pulsewire/pull/28), merged
  `c324c03`: shared runtime config resolution used by both binaries, test
  coverage of CLI/env/XDG/HOME/installed/dev precedence, and server migration
  SQL fallback while retaining legacy config-adjacent SQL.
- **Final Rust CI:** [run 43615939868](https://github.com/sguzman/pulsewire/actions/runs/43615939868)
  passed format, compile, tests, synthetic scheduler backlog, both
  PostgreSQL integration suites, and Clippy.
- **Final container CI:** [run 43615939833](https://github.com/sguzman/pulsewire/actions/runs/43615939833)
  passed clean image builds, non-root/slim-catalog checks and end-to-end smoke.
  The disposable smoke clears config/schema path overrides and mounts
  XDG configs plus installed-share JSON and SQL schemas. It exercises feed
  ingest, administrator bootstrap, and the long-running authenticated HTTP
  server against a throwaway PostgreSQL database.

## Explicit exclusions

No household service or production PostgreSQL/SQLite database was accessed.
No external RSS ingestion, public network ingress or deployment was performed.
Data retention and backup/restore are tracked separately under PW-043;
resource profiling under PW-032; ingress/TLS/security topology under PW-023.
