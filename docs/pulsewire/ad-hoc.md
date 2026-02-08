Good direction. Your current architecture is already close: scheduler/state/backoff/HEAD+GET+etag/last-modified/content-hash are reusable. The cleanest design is to add a second source type, not a separate subsystem.

## Proposed Design (No Code Yet)

### 1. Keep `feeds` as-is, add `watches` (ad hoc pages)
Use the same TOML file pattern:
- existing `feeds` remain unchanged
- add `watches` array for non-RSS pages
- add `watch_defaults` at file level
- extend `global.toml` support to include watch defaults too (you already have folder-level global defaults)

### 2. New watch schema fields
Each watch should support:

- Identity/metadata:
  - `id`, `url`
  - `category`, `provenance`, `tags`, `language`, `content_type`
  - `id_prefix`, `base_poll_seconds`
- Fetch/check behavior:
  - `check_method`: `HEAD | GET`
  - `fallback_to_get`: bool
  - `detectors`: array of `etag | last_modified | content_length | content_hash | element_hash`
  - `fetch_body_on_change`: bool
  - `max_body_bytes`: integer
- Extraction rules:
  - `item_selector`: CSS selector for item/article blocks
  - `item_identity`: one of `href | text | attr`
  - `item_identity_attr`: e.g. `"data-id"` (required when `item_identity=attr`)
  - `title_selector`, `link_selector`, `summary_selector`, `published_selector` (optional)
  - `published_format`: optional parse hint
- Noise reduction / stability:
  - `include_selectors`: optional array (hash only these regions)
  - `exclude_selectors`: optional array (strip ads/nav/etc)
  - `normalize_whitespace`: bool
  - `strip_query_params`: bool
- Emission policy:
  - `emit_mode`: `new_items_only | any_change | digest`
  - `emit_title`: optional override
  - `min_item_count_change`: optional threshold

### 3. What counts as an “item”
For watches, item detection should be explicit and deterministic:
- Parse DOM
- Select nodes via `item_selector`
- Build stable per-item key from `item_identity`
- Hash normalized item payload
- Compare against last successful snapshot
- Emit synthetic entries only for new/changed items according to `emit_mode`

This gives RSS-like behavior without RSS.

---

## Config/Schema Plan

### A. New schema files
- `crates/fetcher/res/schemas/watch.schema.json` (single watch object)
- `crates/fetcher/res/schemas/watch-global.schema.json` (watch defaults)
- Extend `crates/fetcher/res/schemas/feeds.schema.json`:
  - `required` becomes `anyOf: [feeds, watches]`
  - add `watch_defaults`
  - add `watches: [watch.schema]`
- Extend `crates/fetcher/res/schemas/global.schema.json` to include watch default keys.

### B. TOML layout example
- `crates/fetcher/res/feeds/news/site.toml`
  - existing feed defaults + `watch_defaults`
  - mix `[[feeds]]` and `[[watches]]` in same file allowed

---

## Runtime Integration Plan

### 1) Config loading
- Extend raw config structs (`RawFeedsFile`, defaults merge path) to parse `watches`.
- Reuse same precedence:
  - folder `global.toml` -> file defaults -> item overrides

### 2) Scheduler
- Add source kind: `Feed` vs `Watch`
- Reuse same state machine and backoff
- For watch checks:
  - HEAD/GET per settings
  - change decision from configured detectors
  - optional body fetch and extraction

### 3) Persistence
- Prefer minimal schema churn:
  - keep `feeds` table as the canonical source inventory
  - mark source kind (`rss`/`watch`) in config-derived metadata (if needed add column later)
  - convert watch results into synthetic parsed items so downstream API/TUI works unchanged
- Optional future table:
  - `watch_snapshots` for debug diffs/raw extraction audit

### 4) Metrics
Add labels by source kind:
- fetch/check latency by `source_kind`
- parse/extract success/failure counters
- emitted-items counters for watches

### 5) Validation semantics
CLI validation should enforce:
- `item_selector` required for watches
- valid detector combinations
- required companion fields (`item_identity_attr` when needed)
- impossible combos rejected (e.g., `emit_mode=new_items_only` without extractable item identity)

---

## Suggested Rollout Phases

1. Schema + config loading (no scheduler behavior yet)
2. Watch polling with `content_hash` only (safe baseline)
3. DOM extraction + `element_hash` and synthetic item emission
4. Noise controls + robust selectors + validation hardening
5. Metrics + docs + example watch TOMLs

---

## Risks / Decisions to lock now
- Do you want mixed `feeds` + `watches` in same TOML file? (I recommend yes)
- Should watch extraction failure be fatal or soft-fail with retry? (recommend soft-fail + metrics)
- Snapshot retention policy for watch debug data.

---

## SemVer Commit Message (for this planning change, if you commit docs only)
`docs(architecture): design ad hoc watch sources for non-rss site tracking`