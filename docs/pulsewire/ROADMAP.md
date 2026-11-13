# Pulsewire — adoption and resurrection roadmap

> Canonical, scope-frozen engineering plan. Adopted 2026-11-13 on `main` at `c1b1995b13e6`. Weights denote relative full-lifecycle delivery, **not** lines of code or estimated calendar time. Current accepted weighted verification credit: **38/100** (33 source-inventory points, 5 CI/build-gate points); operational readiness remains **unverified**.

## Mission and acceptance contract

Resurrect the **existing Pulsewire** Rust ecosystem as a safe, persistent, multi-user general RSS/news feed ingestion and reading service. Keep its fetcher, server, TUI, SQLite/Postgres architecture, API and source catalog. Finish a useful web reader (already envisioned in the original roadmap) and validate operation on the owner's EndeavourOS/Linux home service environment. Prefer Rust, no npm, preserve existing branding and MIT license, preserve all existing data.

**Default active scope:** news, government, scientific journals, technology, finance, books, comics, and other existing non-excluded feeds. **Default disabled scope:** `youtube` and `podcasts` ingest/scheduling/catalog parsing. Their tracked definitions remain intact; do not delete the YouTube files or previously fetched items. PinePods continues to manage podcasts independently.

**Explicit non-goals for this completion contract:** re-enable giant YouTube/podcast catalogs, aim for million-feed polling rates, replace PinePods, automatic internet exposure, hosted service, marketplace, calendar-based release scheduler, websocket realtime, arbitrary new source integrations, or a dedicated desktop client. These belong to the preserved historical roadmap, not this release's denominator. HTML watches already implemented in Pulsewire must function accurately for explicitly enabled watches.

**Definition of done:** no exploitable account-recovery route, no surprise destructive startup, reproducible builds/CI, bounded fetch resources, correct runtime paths, fair scheduling, working DB persistence, end-to-end fetcher/server/TUI and browser flows, verified backups/restore, user-approved deployment/release. No tag or deployment without applicable authorization.

## Evidence and calibration

- Source examined: Rust workspace (core, fetcher, server, CLI, schemas, TUI), Git tree, latest commits, config and original roadmap; **not yet compiled or executed here**. Last examined commit: `c1b1995b13e6c2b4d8c626cfb6166f487ed06e4b`.
- Source-only milestone completions acknowledge substantial pre-existing implementations without claiming they run correctly. Verification of those components in reality is specifically budgeted again under operational/QA milestones.
- **Adoption baseline 33% (weighted):** PW-001, PW-002, PW-010, PW-011, PW-012, PW-013. All other tasks require the listed objective acceptance evidence. The adoption baseline had no comparable prior percentage. **Round 1 verified current: 38% (+5 pp against the newly established 33% baseline)**, after clean GitHub Actions formatting, compilation, Rust tests and Clippy. This is a weighted task ledger, not a claim of production functionality.
- On every completed round: preserve verified evidence, weights, open blockers, current commit, and next action. Rebaseline transparently when a real scope discovery changes remaining effort.

## Priorities and status

`P0` is a deploy/recovery blocker. `P1` is required for the agreed complete reader product. Statuses: **pending**, **active**, **blocked**, **verified complete**. “Verified complete” on inventory tasks means *source presence verified only*; it is not a runtime assurance.

### M0 — Adoption, scope, baseline

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-001 | 🧭 Source and architecture inventory | 3 | P0 | verified complete | — | Inspect repo HEAD, history, workspace, existing components, risk paths, and old roadmap; store audit evidence. Static inspection complete; runtime unverified. |
| PW-002 | 📌 Adoption and scope contract | 3 | P0 | verified complete | PW-001 | Store canonical scope, explicit exclusions, weighted roadmap, handoff, and old roadmap; verify files committed. |
| PW-003 | 🧪 Reproducible baseline build | 2 | P0 | verified complete | PW-002 | Run the pinned toolchain from clean checkout; record check, lint, fmt, and tests with logs. Not complete until passing or an explained failure is fixed. |

### M1 — Preserve existing working architecture

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-010 | 📡 Fetcher and scheduler implementation inventory | 9 | P0 | verified complete | PW-001 | Code contains RSS/Atom parser, HTTP adapters, adaptive scheduling, per-domain concurrency, backoff and watchdogs. *Source-only verification*; runtime tests are PW-051. |
| PW-011 | 🗄️ Persistence adapter inventory | 6 | P0 | verified complete | PW-001 | Code contains both SQLite and PostgreSQL repositories and schema/migration definitions. *Source-only verification*; durability tested in PW-043/051. |
| PW-012 | 🔌 Multi-user server API inventory | 7 | P0 | verified complete | PW-001 | Code contains route handlers for auth, users, feeds, search, read state, subscriptions, favorites and folders. *Source-only verification*; security fixed in M2. |
| PW-013 | ⌨️ Terminal client implementation inventory | 5 | P0 | verified complete | PW-001 | Code contains TUI screens, keymap, HTTP client and asynchronous refresh. *Source-only verification*; human workflows tested in PW-051. |

### M2 — Security and safe persistence

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-020 | 🚨 Eliminate account-reset takeover | 5 | P0 | active | PW-012 | Block unauthenticated reset-token disclosure immediately; later replace with verified out-of-band/admin recovery, rate limits, and regression tests. No reset token returned to arbitrary requester. |
| PW-021 | 🛡️ Authenticate account lifecycle | 5 | P0 | active | PW-020 | Secure first-admin bootstrap, registration, role/permission model, token revocation on password changes, authentication throttling and consistent unauthorized responses; negative/positive tests pass. |
| PW-022 | 💣 Prevent destructive startup | 5 | P0 | active | PW-011 | Production never wipes data; development reset requires explicit isolated target and force guard. Restore-from-backup and migration replay validated. |
| PW-023 | 🔑 Secrets and network exposure | 3 | P0 | active | PW-021 | No deployable admin/admin, checked-in DB passwords or automatic login; config uses secrets supplied at deployment; minimal listening address/firewall; no credential logs. |
| PW-024 | 🧱 Bound untrusted HTTP and source inputs | 2 | P0 | pending | PW-010 | Enforce download/parse limits and timeouts; validate source URLs and redirect targets; resource-limit and invalid-input tests. |

### M3 — Slim catalog and fair ingestion

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-030 | 🚫 Disable YouTube and podcasts by default | 5 | P0 | active | PW-010 | New install skips the two catalogs before parsing, never schedules them or ingests them; all other categories stay enabled, existing definitions/data are preserved; tests and smoke run prove it. |
| PW-031 | ⏲️ Fair bounded scheduling | 5 | P1 | pending | PW-030 | One category cannot starve others; due-queue paging, domain budgets and retries behave under slow/failing feeds; simulated load tests prove behavior. |
| PW-032 | 📉 Realistic resource envelope | 2 | P1 | pending | PW-031 | Measure CPU, memory, DB growth, poll latency and backlog for the slim news/government/journals catalog; document sustainable bounds on target server. |

### M4 — Build, configuration, operations

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-040 | 📦 Fix container packaging and Compose | 4 | P0 | active | PW-003 | Clean images build with all workspace manifests; Compose paths and network/DNS integration resolve; non-root runtime and resource limits proven with isolated containers. |
| PW-041 | 🧭 Fix runtime schema and config lookup | 3 | P0 | active | PW-040 | Installed and containerized binaries resolve schemas and config from runtime paths, never stale source builds; XDG/default/override precedence tests pass. |
| PW-042 | ✅ CI quality gates | 3 | P0 | verified complete | PW-003 | Main/PR GitHub Actions execute cargo fmt, check, Clippy, unit/integration tests on supported target; all green. |
| PW-043 | 💾 Migrations, retention and backup recovery | 2 | P0 | pending | PW-022 | Both DB backends migrate without loss; read-state persists across restarts; backup/restore and retention procedures validated with fixtures. |

### M5 — Correctness and runtime QA

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-050 | 🧬 Targeted unit and regression tests | 4 | P0 | active | PW-020,PW-030 | Reproduce fixed security, category suppression, change detection, scheduling, malformed feeds, quota and auth edge cases; deterministic offline tests pass. |
| PW-051 | 🔄 End-to-end operational checks | 4 | P0 | pending | PW-040,PW-050 | Real fetch → parse → DB → API → TUI, restart persistence, folder/subscription/read state and schema integration pass under SQLite and PostgreSQL. |
| PW-052 | 🩺 Service health and Prometheus | 2 | P1 | pending | PW-051 | Valid metrics HTTP response and successful Prometheus scrape; readiness checks DB/scheduler as applicable; shutdown and failures visible. |
| PW-053 | 🕸️ HTML-watch change detection | 2 | P1 | pending | PW-024 | Persist comparison hashes; respect CSS extraction/emit modes, body limits, disabled watches and noisy-content suppression; regression tests pass. |

### M6 — Reader product and API stability

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-060 | 📰 Responsive web reader | 4 | P1 | pending | PW-051 | Browser can log in, subscribe, read/search, filter, manage folders/favorites and display errors; Light/Dark/System selector initially System; keyboard/browser QA. |
| PW-061 | 🏷️ Stable API and client semantics | 2 | P1 | pending | PW-051,PW-060 | OpenAPI matches handlers; pagination, unread counts, sorting, auth failures and cursor semantics tested against TUI and browser clients. |

### M7 — Release and home deployment

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-070 | 📚 Documentation, runbook, and compatibility | 1 | P1 | pending | PW-060 | README links correct setup; deployment guide, defaults, privacy, troubleshooting, migration, recovery, and changelog are accurate and checked. |
| PW-071 | 🏁 Verified acceptance and release | 2 | P1 | pending | PW-070,PW-052 | All P0/P1 acceptance checks green; target home-host deployment and backup proven; exact commit recorded; owner accepts release before any tag/publication. |

## Known audit evidence and pitfalls

1. Unauthenticated `/v1/users/password-reset/request` returned a usable reset token directly. The route is being temporarily disabled; secured recovery with a trustworthy delivery/admin process remains PW-020.
2. The fetcher's default config is `mode = "dev"`; its startup can delete SQLite DBs or drop the PostgreSQL fetcher schema. Server seed credentials, TUI auto-login and PostgreSQL passwords are unsafe examples.
3. Current Dockerfiles omit `crates/schemas` from the build inputs and Compose schema mounts point to the wrong repository directory. Runtime schema discovery was compiled from `CARGO_MANIFEST_DIR`.
4. Scheduler loops over categories sequentially. Historical YouTube catalog contains 91 large chunks (~70.8 MB) of source TOML; default 500-second polls with strict per-domain limits are implausible at its scale.
5. HTML-watch content/element change detectors do not compare persistent prior hashes. The maximum response-body setting is defined but not enforced by the HTTP adapter.
6. No checked-in GitHub Actions workflow, no published release; only limited integration tests were located in static inspection.
7. `/health` reports static `ok` and does not check dependencies. The manual metrics HTTP formatting requires a parser/scrape test.
8. Existing DB records from formerly enabled categories may still be visible after disabling their *ingestion*. Never purge them without an explicit migration and operator consent.

## Release stages

1. **Safe offline recovery:** P0 safety fixes, selectively enabled feeds, passing clean build and tests.
2. **Persistent LAN-only service:** functional fetching, server, read/unread and TUI; backups and resource bounds proven.
3. **Complete reader:** browser UX with System/Light/Dark and compatible API; docs and QA.
4. **Owner acceptance:** record exact commit and actual runtime evidence, then obtain separate approval for publication/deployment.

## Original roadmap preservation

The pre-adoption roadmap is preserved verbatim in [ROADMAP-LEGACY.md](ROADMAP-LEGACY.md). Its original Phase 0 core, Phase 1 release tooling and Phase 2 API stabilization are reconciled above. Its Phase 3 browser work is in scope; dedicated desktop and other ambitious future phases remain deferred rather than silently discarded.

## Session checkpoint — 2026-11-13

- **Stable objective:** run safe, slim general news RSS service, not the podcast/YouTube megacatalog.
- **Branch:** main. **Starting HEAD:** `c1b1995b13e6c2b4d8c626cfb6166f487ed06e4b`. **Weights:** 100 total; 33 source-inventory verified plus 5 clean-build/CI verified = 38.
- **Current work:** category-disable control, source-skip tests, emergency account reset endpoint gating, schema path, container build inputs. No deployed service changes.
- **Remaining:** runtime build verification; CI; audit security; integrated and GUI QA; operational acceptance.
- **Next:** inspect CI result and resolve any compiler/test failures; ensure both disabled categories are never ingested in default configuration.

## Verification ledger — 2026-11-13

- Green CI: [Rust checks, run 43493864946](https://github.com/sguzman/pulsewire/actions/runs/43493864946), on source commit [`0f783889`](https://github.com/sguzman/pulsewire/commit/0f7838893e4add34831ecb942da10dc9cd521092).
- Confirmed: `cargo fmt --all -- --check`, `cargo check --workspace --locked`, `cargo test --workspace --locked` (three Rust tests passed), and `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- The disabled-category source-skip test passed. This does **not** establish a live database migration, durable production restart, container-image build, or TUI/browser runtime functionality.
- Weighted change against first adoption ledger: **33% → 38%, +5 percentage points** (`PW-003`, `PW-042` verified). Other items retain their prior statuses.

## Verification ledger — 2026-11-13, round 2

- **Source commit:** [`6f4545c83d61`](https://github.com/sguzman/pulsewire/commit/6f4545c83d619be3911ebcd5ae23e17f1a99012b).
- **Validated CI:** [Rust checks run 43500736801](https://github.com/sguzman/pulsewire/actions/runs/43500736801) succeeded: `cargo fmt --all -- --check`, `cargo check --workspace --locked`, `cargo test --workspace --locked` (**10 tests passed, 0 failed**), `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- **Security changes:** account creation requires a valid administrator token; role migration defaults legacy accounts to non-admin; offline `--bootstrap-admin` requires credentials and does not bind HTTP; automatic dev account seeding removed; password updates revoke all user tokens transactionally.
- **Data safety:** fetcher startup no longer wipes SQLite/PostgreSQL; server refuses startup configured with `dev.reset_on_start = true`; CLI clean requires `--confirm`, `app.mode=dev`, SQLite, `PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET=YES`, a dedicated `PULSEWIRE_DEV_DATA_DIR`, and canonical target-path containment. Files outside this directory, including symlinks escaping it, are rejected.
- **Secrets:** checked-in sample PostgreSQL passwords changed from `admin` to empty; `PULSEWIRE_POSTGRES_PASSWORD` runtime overrides supported by fetcher and server; Compose requires it. Verify PostgreSQL connection handling with punctuation-heavy passwords before release (server still constructs a database URL).
- **Remaining:** secure lost-password recovery, account login throttling, role-management coverage, production database/restore verification, PostgreSQL migrations, Docker image builds, runtime integrations, and local deployment remain **unverified**. Do not deploy yet.
- **Progress:** unchanged **38% → 38% (+0 pp)**. This round made substantial verified progress *inside unfinished weighted tasks* PW-020/PW-021/PW-022/PW-023/PW-050, but none meets its full acceptance criteria yet; no partial credits are invented. These statuses are now active where touched.
- **Next:** review and harden SQLx PostgreSQL connection handling so secret values with URL-reserved characters work; integrate automated PostgreSQL runtime tests and verify container builds without exposing the API.
