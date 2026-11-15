# PW-061 — Stable API and client semantics acceptance

**Accepted 2026-11-15:** PW-061 verified complete, **2/2 weighted points**, Pulsewire overall **90/100 → 92/100 (+2 percentage points)**. This is CI and source-contract acceptance, **not** owner approval for home deployment or release.

## Source and exact-head verification

- [PR #55](https://github.com/sguzman/pulsewire/pull/55) merged as [`e50ffc38`](https://github.com/sguzman/pulsewire/commit/e50ffc38eaa17049a6c05bec0104442bde3f95a7).
- Exact PR source head tested: `759c7936d7842f27a2c9dc641a27fa8a84c6dbde`.
- [Rust + reader CI 43759054529](https://github.com/sguzman/pulsewire/actions/runs/43759054529): successful nightly rustfmt, workspace build/tests, synthetic scheduler, PostgreSQL service and watch acceptance, Clippy; independent Node route/schema contract and desktop/mobile Chromium smoke passed.
- [Container CI 43759054590](https://github.com/sguzman/pulsewire/actions/runs/43759054590): successful non-root fetcher/server image build and disposable PostgreSQL+SQLite RSS → server → API → TUI/browser/restart/backup smoke. The job log explicitly contains:
  - `PW-061 live API contract passed on http://127.0.0.1:18091` (PostgreSQL)
  - `PW-061 live API contract passed on http://127.0.0.1:18092` (SQLite)

## Verified contract matrix

| Area | Verified behavior |
|---|---|
| HTTP routes and authorization | All 47 router method/path registrations match the checked-in OpenAPI specification, including service readiness and browser assets. Bearer-protected routes document 401; unauthenticated requests to entries/counts return 401. The old insecure public password-reset definitions are absent from the published schema. |
| Pagination | Global, per-feed, folder and search listings sort descending by feed-item insertion ID. A one-row lookahead proves the existence of a following page before exposing `next_offset`; empty/last pages return `null`. `limit=0` clamps to 1, default 50, maximum 200. |
| Cursor/high-watermark | `next_cursor` is the highest ID **in this response's item page**, or `null` when empty. `since=N` returns only item IDs greater than N and echoes the filter. For older pages, clients use `next_offset`, never `next_cursor`. |
| Search | Title/summary/description search is case-insensitive literal substring matching. `%`, `_` and backslash do not become wildcard match operators, verified on both SQL dialects. Invalid `read` filters produce HTTP 400. |
| Counts and read state | The SQLite `es.read_at_ms` reference was corrected to `es.read_at`. Full-catalog `/v1/feeds/counts` returns expected totals; read/unread changes update per-user counts. `/v1/entries/unread/count` is restricted to subscribed feeds; subscribe/unsubscribe and mark read/unread are verified. |
| Client integration | The TUI `next_offset` page transition and Chromium desktop/mobile reader are exercised in CI. Browser now follows actual `next_offset`, not the inferred length of the returned page. Folder creation, feed assignment, pagination, removal and deletion are exercised on both backends. |

## Explicit limitations and safety boundary

This contract defines **ID-based sorting**, not date sorting; a new ingestion can shift numeric offsets between requests. The `next_cursor` value is page-local, not a frozen snapshot or older-page continuation token. Count endpoints intentionally have different scopes: subscription-scoped global unread totals versus full catalog per-feed totals. SQL query tests use two-item synthetic RSS fixtures on disposable databases; broader concurrent-ingestion pagination and real-host resource benchmarking are separate work.

No actual household SQLite/PostgreSQL database, network, CoreDNS, Docker service, secrets, media files, deployment, public host exposure, release or Git tag was modified. PW-023 (trusted ingress/TLS/secrets), PW-032 (target-host resource envelope), PW-070 (final runbook/doc quality) and PW-071 (approved home deployment/release) remain open.
