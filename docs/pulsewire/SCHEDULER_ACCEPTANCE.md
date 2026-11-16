# PW-031 — fair, bounded scheduler acceptance

**Evidence date:** 2026-11-14. This record concerns source, tests and isolated CI only, **not** a deployed home service. See [ROADMAP.md](ROADMAP.md) for weighted scope and the separate PW-032 target-host resource envelope.

## Acceptance matrix

| Property | Implementation / verification | Outcome |
|---|---|---|
| No category holds up another category's timer or retry backoff | [PR #12](https://github.com/sguzman/pulsewire/pull/12): independently polled category loops, per-category processing windows, shared global/domain request guards | Verified |
| Waiting for a busy domain cannot consume an idle global request permit | PR #12: acquire domain permit before global permit; concurrency regression | Verified |
| Blocked or failed due-query cannot stall the healthy category | [PR #13](https://github.com/sguzman/pulsewire/pull/13): offline actual scheduler entry-point tests | Verified |
| A slow HTTP GET cannot starve an unrelated category | [PR #16](https://github.com/sguzman/pulsewire/pull/16): intentionally stalled fake GET and successful healthy GET | Verified |
| A failed HTTP GET does not cascade and persists error backoff | [PR #18](https://github.com/sguzman/pulsewire/pull/18): Timeout result, ErrorBackoff vs. healthy NeedsHead state | Verified |
| Retry deadlines and recovery remain correct | [PR #20](https://github.com/sguzman/pulsewire/pull/20): exponential/capped backoff, no early retry and recovery to GET after a changed HEAD | Verified |
| Due feed batches have stable ordering | [PR #15](https://github.com/sguzman/pulsewire/pull/15): secondary feed ID sort key in both SQL adapters | Verified |
| Bounded selection and deferral with >1,000 real SQL feed fixtures | [PR #17](https://github.com/sguzman/pulsewire/pull/17) (PostgreSQL TEMP tables) and [PR #21](https://github.com/sguzman/pulsewire/pull/21) (in-memory SQLite) each exercise 1,101 feeds, 1,000-item first page, deferral, error exclusion and the remaining feeds | Verified |
| Categories progress under repeated synthetic backlog | [PR #19](https://github.com/sguzman/pulsewire/pull/19): 3 categories, 1,075 feeds/category and 3 sweeps; count, page limits, zero reprocessing and peak concurrency asserted | Verified |

## CI evidence

- Independent loops and shared caps: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43553769950), [container](https://github.com/sguzman/pulsewire/actions/runs/43553769955) — passed.
- Blocked/failing queries and bounded offline paging: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43604223443), [container](https://github.com/sguzman/pulsewire/actions/runs/43604223347) — passed.
- Stable SQL tie-breaking and SQLite small-page selection: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43604871765), [container](https://github.com/sguzman/pulsewire/actions/runs/43604871755) — passed.
- Stalled HTTP isolation: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43605301505), [container](https://github.com/sguzman/pulsewire/actions/runs/43605301559) — passed.
- PostgreSQL 1,101-feed queue: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43607506288), [container](https://github.com/sguzman/pulsewire/actions/runs/43607506295) — passed.
- Failed HTTP state isolation: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43607702167), [container](https://github.com/sguzman/pulsewire/actions/runs/43607702238) — passed.
- Rebased three-category synthetic backlog: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43609054897), [container](https://github.com/sguzman/pulsewire/actions/runs/43609054901) — passed. This integrated the previous PostgreSQL and HTTP regressions on the same PR branch.
- Retry and recovery state-machine checks: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43608460798), [container](https://github.com/sguzman/pulsewire/actions/runs/43608460788) — passed.
- SQLite 1,101-feed queue: [Rust](https://github.com/sguzman/pulsewire/actions/runs/43609386798), [container](https://github.com/sguzman/pulsewire/actions/runs/43609386787) — passed.

**Final integrated acceptance passed:** [PR #22](https://github.com/sguzman/pulsewire/pull/22) merged as [63dc19b](https://github.com/sguzman/pulsewire/commit/63dc19bd07343746d0096f6ed2c3fa3396181cca). Both [Rust checks 43610317767](https://github.com/sguzman/pulsewire/actions/runs/43610317767) and [container packaging and smoke 43610317718](https://github.com/sguzman/pulsewire/actions/runs/43610317718) succeeded on the integrated source, including the PostgreSQL due-queue and synthetic-load checks. PW-031 is verified complete: **5/5 weighted points**, raising the roadmap total **60/100 to 65/100**.

## Reproducible synthetic measurement

The rebased PR #19 GitHub Actions Rust job recorded:

| Metric | Observed |
|---|---:|
| Synthetic feed actions | 3,225 |
| Categories | 3 |
| Complete scheduling rounds | 3 |
| Maximum batch size per category tick | 1,000 |
| Configured shared global request limit | 4 |
| Measured peak concurrent HTTP requests | 3 |
| Measured elapsed wall time | 2,291 ms |
| Feeds missing or repeatedly processed in fixture | 0 |

The load runs against injected in-process fake HTTP/Repo interfaces, with a deliberate 1 ms simulated HTTP response delay and no external RSS calls. Times depend on CI runner load and are **not** production RSS throughput, a real latency SLO, or a host capacity forecast. A dedicated CI step reruns the test with `--nocapture` and logs these values.

## Running tests

Normal offline checks (includes SQLite and synthetic scheduling):

```sh
cargo fmt --all -- --check
cargo test -p pulsewire-core --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

For the ignored PostgreSQL test, use only an isolated CI-equivalent PostgreSQL service with the exact expected disposable database, user and locally supplied password; never point it at a production database:

```sh
cargo test -p pulsewire-core postgres_due_queue_paging_and_errors --locked -- --ignored --test-threads=1
cargo test -p pulsewire-core sustained_multicategory_queue_is_bounded --locked -- --nocapture --test-threads=1
```

The first command requires `PULSEWIRE_TEST_POSTGRES_PASSWORD` in the private environment. Its fixture uses session-scoped PostgreSQL TEMP tables rather than writing application tables.

## Out of scope / remaining gates

- **PW-032** must independently measure CPU, memory, database growth, true poll latency and backlog on the actual slim catalog under representative target-server hardware and network conditions. The existing synthetic test does **not** satisfy that item. The separate [offline PW-032 envelope reporter](RESOURCE-ENVELOPE.md) can validate future samples, but cannot substitute for approved host measurements.
- **PW-023 / PW-040 / PW-041 / PW-043 / PW-051** still govern secrets, ingress/network isolation, packaging, runtime paths, backups and full reader end-to-end correctness.
- No production database migration, home Docker network modification, CoreDNS change, external RSS polling, actual-host deployment, or release was performed for PW-031.
