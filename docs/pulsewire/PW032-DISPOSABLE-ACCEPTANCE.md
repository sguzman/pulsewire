# PW-032 — observed disposable CI resource sample acceptance

**Verified code-only/small-workload test, 2026-11-15.** This is **not**
the required target-host resource envelope. PW-032 remains **pending
0/2 weighted points**. The project's accepted score remains
**93/100 (Δ +0)**.

## Source and exact-head CI

- [PR #63](https://github.com/sguzman/pulsewire/pull/63)
  squash-merged at [`a6d7a4c852e649e1d155777ce5f7d6d4bc4d329e`](https://github.com/sguzman/pulsewire/commit/a6d7a4c852e649e1d155777ce5f7d6d4bc4d329e).
- Exact PR head tested: `57c904fe940b96135c3385ed5a4d3cf9ae07c918`.
- [Rust/docs/reader CI 43792770359](https://github.com/sguzman/pulsewire/actions/runs/43792770359):
  **success**, including Rust formatting, workspace build/tests,
  PostgreSQL/scheduler regressions and Clippy, documentation/browser
  checks, no-npm Docker/Prometheus normalization regressions.
- [Container CI 43792770308](https://github.com/sguzman/pulsewire/actions/runs/43792770308):
  **success**, including non-root images, real disposable PostgreSQL/SQLite,
  source-backed RSS→API/TUI/reader, file-backed passwords, authenticated
  Prometheus and isolated TLS/ingress regressions. The new measured
  sampler succeeded within that run.
- CI log marker: `PW-032 disposable CI observations passed: 3 real snapshots, 1 private fixture feeds, CPU max 0.18%, memory max 3702522 bytes; no host acceptance`.

## What the measured sampler verified

| Source | Isolated test contract |
|---|---|
| Docker stats | Three real CPU%/memory-use samples from a named running container, still capped at 0.75 CPUs and 512 MiB by the workflow |
| PostgreSQL | Read-only `pg_total_relation_size` sum of fetcher-schema ordinary/materialized tables, including indexes/TOAST, not a shared database total or WAL |
| Due backlog | Read-only `fetcher.feed_state_current` rows whose `next_action_at <= CURRENT_TIMESTAMP`, not the scheduler's last observed gauge |
| HTTP metrics | Existing private bearer-protected `/metrics` endpoint, with HEAD/GET action counters and complete aggregate Prometheus cumulative latency buckets |
| Sanitization | Remove Docker container identifiers and metric domain labels, parse Docker stats units, reject unknown fields and malformed histograms |
| CI boundary | Hard check `GITHUB_ACTIONS=true`, explicit `PULSEWIRE_PW032_DISPOSABLE=YES`, exact test container names and `pulsewire-ci` network, and no host-published metrics port |
| Acceptance guard | Same offline reporter marks `target_host_verified:false`, `sustainable_capacity_proven:false`, `deployment_approved:false`, `pw032_weighted_points:0` |

### Observed fixture measurements

| Metric | Observed in this disposable CI run |
|---|---:|
| Snapshot count | 3 |
| Fixture feeds | 1 local news RSS |
| Sampled Docker CPU maximum | 0.18% |
| Sampled Docker-reported memory maximum | 3,702,522 bytes |

These are **not** benchmark thresholds or a representative workload.
The run does not claim a true sustained HTTP p95, target-host CPU
headroom, full three-category feed throughput, realistic database
growth rate, or due-queue stability under contention.

## Still outstanding

[Issue #62](https://github.com/sguzman/pulsewire/issues/62)
requires approved target-host measurements for the real slim
news/government/journals catalog over representative idle/busy,
network-failure and household-contention windows, using explicit
Pulsewire-owned database accounting and retention. PW-023 requires
real-host access controls/TLS and PW-071 requires separate owner
approval before any deployment or tag.

**No household database, CoreDNS, firewall, Docker network,
running service, real feed, credentials, monitoring stack, or
release/tag was accessed or modified by this CI work.**
