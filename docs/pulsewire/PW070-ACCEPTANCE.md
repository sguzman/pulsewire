# PW-070 — documentation, runbook, and compatibility acceptance

**Verified complete, 2026-11-15: 1/1 weighted point.**
Canonical project score: **92/100 → 93/100 (Δ +1 percentage point)**.
This acceptance validates source-aligned operator documentation and isolated CI
regressions, **not** a home-host deployment, migration, or authorization to tag.

## Source and exact-head verification

- [PR #56](https://github.com/sguzman/pulsewire/pull/56), merged as
  [`e7811b107917bcdd524ca3fdbc8925ca65890618`](https://github.com/sguzman/pulsewire/commit/e7811b107917bcdd524ca3fdbc8925ca65890618).
- Tested PR source head: `26aeffdfa692133e4b1d5dce5e1e396dbb2d96f5`.
- [Rust checks, browser, and documentation 43762182792](https://github.com/sguzman/pulsewire/actions/runs/43762182792):
  **success**. Rust formatting, full workspace build/tests, scheduler load,
  PostgreSQL service/due-queue/watch acceptance and Clippy passed; the reader
  browser and OpenAPI contract job stayed green.
- New `docs` CI job (Node standard library, **no npm**) passed syntax and
  source-backed operator checks: **12 documents and 50 local Markdown links**.
  It verifies claimed README `just` targets, CLI destructive-cleanup guards,
  preserved startup data, release safety, config/Compose network and bind defaults,
  non-root container target, disabled feed categories/metrics and TUI credentials.
- [Container CI 43762182788](https://github.com/sguzman/pulsewire/actions/runs/43762182788):
  **success**. Existing non-root image packaging and disposable PostgreSQL/
  SQLite reader/API/TUI, backup and recovery tests remained green.

## Acceptance matrix

| Acceptance area | Source-checked result |
|---|---|
| README setup and working links | Corrected outdated tooling/release paths and removed nonexistent `just post-change`. Local Markdown references checked in CI. |
| Deployment and defaults | Compose requires explicit existing external Docker network, overridable PostgreSQL hostname, private password, loopback-only host-published server; internal server listener and disabled/unpublished metrics are distinguished. |
| Privacy and threat boundaries | Browser per-tab bearer-token storage, sensitive database/backups/cookie/header files, secrets/log limitations, untrusted forwarding headers and private-fetch opt-in are documented with explicit PW-023 blockers. |
| Compatibility | CI-verified x86_64 musl images, nightly local toolchain, PostgreSQL/SQLite, browser/TUI validation scopes, runtime config/schema path precedence and known incompatibility limits are documented. |
| Recovery, migrations and retention | Copy-only restore rehearsal for SQLite/PostgreSQL; no in-place downgrade guarantee, no destructive startup, no approved age-based pruning; actual host recovery remains unverified. |
| Troubleshooting | Non-mutating Compose checks, separate `/health` and `/ready` semantics, auth/429, subscriptions/counts, fetch/egress, and release/restore blocker troubleshooting guide added. |
| Release/changelog | Changelog updated; release guide explicitly warns root Just release recipes automatically execute, push and tag. PW-071 still requires owner approval and independent deployment gates. |

## Boundary and next gates

No household database, services, router, CoreDNS, Docker network, credentials,
storage, source catalogs, production metrics, reverse proxy or public port were
changed. No version was bumped or tag created. **The remaining 7/100** are:
PW-023 (3, trusted ingress/TLS/secrets), PW-032 (2, realistic target-host
resource envelope) and PW-071 (2, approved real-host acceptance/release).
Those cannot be inferred from the successful documentation tests.
