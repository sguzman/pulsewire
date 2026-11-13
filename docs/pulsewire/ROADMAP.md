# Pulsewire — adoption and resurrection roadmap

> Canonical, scope-frozen engineering plan. Adopted 2026-11-13 on `main` at `c1b1995b13e6`. Weights denote relative full-lifecycle delivery, **not** lines of code or estimated calendar time. Current accepted weighted verification credit: **48/100** (33 source-inventory, 5 CI/build, 5 slim catalog, 5 destructive-startup safety); operational readiness remains **unverified**.

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
| PW-022 | 💣 Prevent destructive startup | 5 | P0 | verified complete | PW-011 | Production never wipes data; development reset requires explicit isolated target and force guard. Restore-from-backup and migration replay validated. |
| PW-023 | 🔑 Secrets and network exposure | 3 | P0 | active | PW-021 | No deployable admin/admin, checked-in DB passwords or automatic login; config uses secrets supplied at deployment; minimal listening address/firewall; no credential logs. |
| PW-024 | 🧱 Bound untrusted HTTP and source inputs | 2 | P0 | pending | PW-010 | Enforce download/parse limits and timeouts; validate source URLs and redirect targets; resource-limit and invalid-input tests. |

### M3 — Slim catalog and fair ingestion

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-030 | 🚫 Disable YouTube and podcasts by default | 5 | P0 | verified complete | PW-010 | New install skips the two catalogs before parsing, never schedules them or ingests them; all other categories stay enabled, existing definitions/data are preserved; tests and smoke run prove it. |
| PW-031 | ⏲️ Fair bounded scheduling | 5 | P1 | pending | PW-030 | One category cannot starve others; due-queue paging, domain budgets and retries behave under slow/failing feeds; simulated load tests prove behavior. |
| PW-032 | 📉 Realistic resource envelope | 2 | P1 | pending | PW-031 | Measure CPU, memory, DB growth, poll latency and backlog for the slim news/government/journals catalog; document sustainable bounds on target server. |

### M4 — Build, configuration, operations

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-040 | 📦 Fix container packaging and Compose | 4 | P0 | active | PW-003 | Clean images build with all workspace manifests; Compose paths and network/DNS integration resolve; non-root runtime and resource limits proven with isolated containers. |
| PW-041 | 🧭 Fix runtime schema and config lookup | 3 | P0 | active | PW-040 | Installed and containerized binaries resolve schemas and config from runtime paths, never stale source builds; XDG/default/override precedence tests pass. |
| PW-042 | ✅ CI quality gates | 3 | P0 | verified complete | PW-003 | Main/PR GitHub Actions execute cargo fmt, check, Clippy, unit/integration tests on supported target; all green. |
| PW-043 | 💾 Migrations, retention and backup recovery | 2 | P0 | active | PW-022 | Both DB backends migrate without loss; read-state persists across restarts; backup/restore and retention procedures validated with fixtures. |

### M5 — Correctness and runtime QA

| ID | Item (deliverable) | Weight | Priority | Status | Dependencies | Objective acceptance criteria |
|---|---|---:|---|---|---|---|
| PW-050 | 🧬 Targeted unit and regression tests | 4 | P0 | active | PW-020,PW-030 | Reproduce fixed security, category suppression, change detection, scheduling, malformed feeds, quota and auth edge cases; deterministic offline tests pass. |
| PW-051 | 🔄 End-to-end operational checks | 4 | P0 | active | PW-040,PW-050 | Real fetch → parse → DB → API → TUI, restart persistence, folder/subscription/read state and schema integration pass under SQLite and PostgreSQL. |
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

## Verification ledger — 2026-11-13, round 3 (PostgreSQL runtime)

- **Verified source:** [`35551a5ab285`](https://github.com/sguzman/pulsewire/commit/35551a5ab285d43418540424f6f386459d759234).
- **CI evidence:** [successful GitHub Actions run 43505774415](https://github.com/sguzman/pulsewire/actions/runs/43505774415). Rust formatting, workspace compilation, unit tests, isolated PostgreSQL 16 integration, and Clippy with `-D warnings` all passed. The regular suite had **11 passing tests plus one deliberately ignored container-only test**, which ran separately and passed (**12 executed tests, 0 failures**).
- **Connection safety:** the server now uses SQLx `PgConnectOptions` with separate host, port, user, password, database and SSL-mode fields; it never embeds raw credentials into a connection URL. The fetcher already used structured SQLx options. Unsupported TLS modes are rejected explicitly.
- **PostgreSQL test:** exercised a disposable PostgreSQL service with reserved characters in the password, the prior-to-admin-role `users` schema, repeatable server schema migration, preservation of legacy user IDs and non-admin status, password-verified administrator promotion, authenticated reader creation, and password rotation that invalidates the old session and issues a fresh token.
- **Safety boundary:** the CI database is disposable and isolated. The test does not access any home PostgreSQL instance or real credentials. Production PostgreSQL backups, deployability, and actual LAN service behavior are **not** yet demonstrated.
- **Task accounting:** PW-051 is now **active**, not complete; migrations, persistence through restart, other API/TUI flows, and both storage backends still require dedicated evidence. PW-041, PW-043 and PW-050 also have remaining work. **Weighted overall unchanged at 38% (+0 percentage points)** because no whole milestone item met its full acceptance criteria.
- **Next verified priorities:** validate clean container image builds; add isolated SQLite and PostgreSQL restart/backup/restore tests; check secure operator bootstrap and remaining authentication abuse protections. Keep YouTube/podcasts disabled and catalogs preserved.

## Verification ledger — 2026-11-13, round 4 (container packaging and networking)

- **Source commit:** [`7cd7b9e1e06c`](https://github.com/sguzman/pulsewire/commit/7cd7b9e1e06c064422d8eadd627bf6b63be3cdca).
- **Clean CI:** [Rust checks 43509567029](https://github.com/sguzman/pulsewire/actions/runs/43509567029) and [container/Compose smoke 43509567038](https://github.com/sguzman/pulsewire/actions/runs/43509567038) **both succeeded**.
- **Build proof:** two `rust:1.91-alpine` → musl/`scratch` release images built with `cargo build --locked` and all workspace manifests. Both images identified numeric non-root user `65532:65532`; the default fetcher image retains news feeds but omits the historical `youtube.com` and `podcasts` directories. Git retains the original source catalogs intact.
- **Runtime smoke proof:** on a disposable PostgreSQL 16 Docker network, the fetcher ran its one-feed benchmark/database initialization; the server ran offline admin bootstrap, started and answered `/health`, accepted authenticated login, and rejected anonymous account creation. The server port was bound only to `127.0.0.1` on the CI host. The test uses a **non-default PostgreSQL DNS alias** to verify the new `PULSEWIRE_POSTGRES_HOST` override works in both binaries.
- **Compose proof:** both sample Compose files passed `docker compose config --quiet`, now require an explicit existing `PULSEWIRE_DOCKER_NETWORK`, use the supplied PostgreSQL host and password, remove obsolete hard-coded DNS IPs, and publish only loopback host ports. The images supply their own versioned schemas instead of bind-mounting source checkout schemas.
- **Operator runbook:** [DEPLOYMENT.md](DEPLOYMENT.md) explicitly warns against deploying or migrating existing data before validated backup/restore; no host services or stored user data were changed during this round.
- **Acceptance still open:** PW-040 requires runtime testing under the Compose CPU/memory caps and a repeatable real upgrade; PW-041 requires native installed-binary/XDG path QA; PW-030 requires a live scheduler proof with disabled directories mounted; PW-043/PW-051 require persistence, backup/restore, real RSS fetch → API/TUI, and broader data checks. The current smoke test's synthetic benchmark is **not** evidence of real feed retrieval.
- **Progress ledger:** **38% → 38% (+0 percentage points)**. PW-030, PW-040, PW-041 and PW-051 advanced with passing evidence, but none has met its full definition of done; do not invent partial weighted completion.
- **Next:** isolated real-RSS fixture through fetcher → PostgreSQL → API, verify disabled categories don't ingest even when mounted, exercise Docker CPU/memory constraints, and add reversible SQLite/PostgreSQL backup/restore fixtures. Continue to exclude podcasts and YouTube by default.

## Verification ledger — 2026-11-13, round 5 (live RSS and recovery)

- **Source test:** [`6702fafd8eca`](https://github.com/sguzman/pulsewire/commit/6702fafd8ecada0c56a39b07482e138c7cc08afa), with a workflow YAML indentation correction [`23f60e0cbbef`](https://github.com/sguzman/pulsewire/commit/23f60e0cbbefa3a644b425b38d91587a24589ccc).
- **Passing CI:** [Rust check suite 43511740707](https://github.com/sguzman/pulsewire/actions/runs/43511740707) and [container pipeline 43511740725](https://github.com/sguzman/pulsewire/actions/runs/43511740725) both completed successfully.
- **Real RSS integration:** disposable `nginx:alpine` served checked-in `tests/fixtures/rss/feed.xml` inside the isolated CI Docker network. The actual live Pulsewire fetcher, under 512 MiB memory/0.75 CPU/pid limits, scheduled an initial HTTP GET, parsed two RSS stories, and persisted their data in PostgreSQL; the authenticated server's `GET /v1/entries?feed_id=...` returned them. This replaces the earlier benchmark-only evidence; the fixture is local/deterministic and does not access news websites.
- **Disabled-category acceptance:** Pulsewire's built image excludes both retired source directories; a separate mounted feed tree included deliberately malformed TOML under `youtube.com` and `podcasts`. The live fetcher parsed the news feed and stored articles successfully despite those malformed disabled files; no feeds in the disabled categories were inserted. Original catalog files stay in Git and no purge occurs. This plus source loader/scheduler exclusion and the earlier Rust unit test satisfies **PW-030**.
- **Durability evidence:** API-marked article read state survived a server restart. The job used `pg_dump -Fc` to capture the disposable PostgreSQL database, created **new** `data_restore`, ran `pg_restore --exit-on-error`, and verified both fetched articles and read-state records in the restored database. Source `data` was not dropped/overwritten.
- **Resource envelope:** both images were built from scratch, operated as numeric non-root UID/GID 65532, and both live service containers ran under their defined CPU/RAM/pid caps; ports stayed loopback-bound. Clean builds, Compose syntax and runtime hostname override were previously green.
- **Milestone statuses:** PW-030 **verified complete**, PW-043 **active** (Postgres backup/restore verified but SQLite, migration upgrades and retention remain), PW-040 **active** (still reserving repeated upgrade/deployment QA), PW-051 **active** (TUI/real-world scenarios and both backends outstanding).
- **Weighted completion:** **38% → 43% (+5 percentage points)**. The five points represent exactly PW-030; no partial credit for remaining milestones.
- **Next:** exercise SQLite backup/restore and repeated upgrade migrations in isolated CI, add a restart/recovery drill for both service databases, and continue authenticating rate-limiting/reader API operations. No operator deployment authorization has been given.

## Verification ledger — 2026-11-13, round 6 (SQLite recovery and safe startup)

- **Source commit:** [`59e36406e286`](https://github.com/sguzman/pulsewire/commit/59e36406e286a54eb35b58804fe2228db66119f7) includes a SQLite file-backed recovery test and reader API smoke additions.
- **Verified Rust CI:** [run 43519275262](https://github.com/sguzman/pulsewire/actions/runs/43519275262) succeeded in all steps: format, compile, workspace Rust tests, isolated PostgreSQL integration, and Clippy `-D warnings`. The new `legacy_sqlite_schema_backup_restore_preserves_reader_state` test passed.
- **SQLite recovery evidence:** create the old `users` schema without an admin column, preserve a fixed user ID through repeated schema migrations, verify non-admin default and credential-gated promotion; create an article with authenticated read state, snapshot using SQLite `VACUUM INTO`, restore to a separate file, replay migrations twice, and retrieve the original article and read state through reader handlers. No original file was overwritten.
- **PostgreSQL recovery evidence:** [round 5 container CI](https://github.com/sguzman/pulsewire/actions/runs/43511740725) successfully restored a new database from `pg_dump -Fc` and validated article and read-state records.
- **Destructive-operation safety:** normal fetcher and server startup preserve data; server rejects `dev.reset_on_start`; CLI cleanup requires a designated nested development root, SQLite, explicit confirmation, and `PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET=YES`. Existing tests reject paths outside the root and symlink escapes. No production reset occurs on startup.
- **PW-022 is verified complete:** the explicit startup/cleanup and two-backend restore-and-migration acceptance criteria now have passing evidence. **Weighted progress: 43% → 48% (+5 percentage points)**.
- **Still open:** PW-043 requires retention and additional upgrade/migration recovery procedures. PW-051 needs TUI and broader reader integration. Expanded container API smoke is separately tracked by [run 43519275224](https://github.com/sguzman/pulsewire/actions/runs/43519275224); do not claim it passed before it finishes.
- **Next:** complete the expanded API smoke and repair any errors; then cover retention and production migration repeatability, followed by remaining account abuse prevention. Continue keeping YouTube and podcast ingestion disabled.

## Verification ledger — 2026-11-13, round 6 addendum (reader API smoke)

- **Verified container CI:** [run 43519275224](https://github.com/sguzman/pulsewire/actions/runs/43519275224) **completed successfully** on source commit [`59e36406e286`](https://github.com/sguzman/pulsewire/commit/59e36406e286a54eb35b58804fe2228db66119f7). Compose validation, clean image builds, non-root/slim catalog assertions, and the isolated PostgreSQL smoke step were green.
- **Reader API checks:** anonymous `GET /v1/entries` was rejected with HTTP 401; authenticated subscription and favorite creation succeeded; search returned the two fixture articles; read/unread filtering tracked an article marked read.
- **Recovered reader state:** the CI job backed up the disposable PostgreSQL database using `pg_dump`, restored into a different database with `pg_restore`, and verified that subscriptions and favorites survived alongside the previously tested articles and read-state rows. The production/home database was not contacted.
- **Milestone accounting:** PW-051 still requires end-to-end TUI verification and both storage-backed user workflows; PW-043 still requires retention and broader migration/backfill checks. **Overall remains 48% (+0 pp)** after this addendum.
- **Known follow-up:** improve server startup ordering by rejecting `dev.reset_on_start` *before* database connection/schema migration, even though the flag cannot currently trigger a wipe.

## Verification ledger — 2026-11-13, round 7 (operator recovery)

- **Implementation:** [`1278ffb97811`](https://github.com/sguzman/pulsewire/commit/1278ffb9781107d094a3f1841a7638c06beae611) consolidates source commits adding `pulsewire-server --recover-user` plus a SQLite regression. Operator recovery requires `PULSEWIRE_RECOVERY_CONFIRM=RECOVER_USER`, `PULSEWIRE_RECOVERY_USERNAME`, and `PULSEWIRE_RECOVERY_PASSWORD` (minimum 12 bytes); it never enables the unsafe unauthenticated HTTP recovery endpoint.
- **SQLite behavior verified:** transactional password change for an existing account; missing users and too-short passwords rejected without modifying sessions; both `user_tokens` and old `user_password_resets` deleted atomically; existing user ID and administrator flag preserved; old password invalid and new password verifies.
- **Green Rust CI:** [run 43522184835](https://github.com/sguzman/pulsewire/actions/runs/43522184835) passed formatting, compilation, regular unit/integration tests, isolated PostgreSQL integration (existing suite), and Clippy. The new `db::bootstrap_tests::offline_recovery_rotates_password_and_revokes_both_token_types` test passed.
- **Scope caveats:** no web password reset is available; the new recovery command has **not yet** been exercised against PostgreSQL, nor called as a deployed CLI in container smoke. CI [container run 43522184870](https://github.com/sguzman/pulsewire/actions/runs/43522184870) is still building at this checkpoint.
- **Open acceptance:** PW-020 and PW-021 remain active; authentication rate limiting and a fully tested secure recovery workflow are still required. The inherited previous source-only inventory weights have not changed. **Total verified completion remains 48% (+0 pp)**.
- **Next:** validate `--recover-user` end-to-end in disposable PostgreSQL via container smoke and add a PostgreSQL token-revocation regression; then bound login abuse with documented limits. Preserve original user data and keep YouTube/podcasts disabled.

## Verification ledger — 2026-11-13, round 8 (PostgreSQL offline recovery)

- **Verified source:** [`94ab528414e9`](https://github.com/sguzman/pulsewire/commit/94ab528414e939dbc11a8195dc55d46b0f9fa5f5). Both [Rust checks 43525939594](https://github.com/sguzman/pulsewire/actions/runs/43525939594) and [container smoke 43525939629](https://github.com/sguzman/pulsewire/actions/runs/43525939629) succeeded with formatting, compilation, unit tests, isolated PostgreSQL integration, Clippy, release images, and the live container smoke.
- **PostgreSQL regression:** verifies operator recovery for an existing admin after an ordinary password change. The old bearer token is invalidated, `user_tokens` and `user_password_resets` are cleared, the old password fails, the new password works, and the user's administrator role is preserved. Unknown users and too-short passwords are rejected.
- **Container command acceptance:** disposable PostgreSQL 16; `pulsewire-server --recover-user` with an invalid `PULSEWIRE_RECOVERY_CONFIRM` fails and leaves the original hash unchanged. Successful offline execution exits without starting another HTTP listener; its effects are observed by the already-running API server. The prior bearer token and password are rejected, a new login succeeds, reset-token rows disappear, and the role stays admin.
- **Safety refinement:** confirmation, operator username, and password length are validated before connecting to a database or applying migrations. No public reset-token route was added; SQL updates and session/reset-token revocation share one database transaction. YouTube/podcast categories remain disabled.
- **Acceptance still open:** PW-020/PW-021 require abuse-resistant login throttling, coverage for concurrent/wrong-account scenarios, and deployment-specific operator authorization controls. PW-023 and real-host backup/restore/deployment also remain open. **Weighted completion remains 48% (+0 percentage points)** because no additional task meets its full acceptance criteria.
- **Next:** implement and verify login throttling (including invalid usernames, concurrency, session reset behavior and denial responses) before revisiting PW-020/PW-021 acceptance. Maintain a locked-down default listener and leave home services untouched.
