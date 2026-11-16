# PW-032 — synthetic resource reporter acceptance (partial)

**Code-only reporter verified 2026-11-15; actual PW-032 resource-envelope
acceptance still PENDING (0/2). Overall Pulsewire 93/100 (Δ +0).**
No household host, service, database or real feed was measured.

## Exact-head source and CI evidence

- [PR #61](https://github.com/sguzman/pulsewire/pull/61)
  merged as [`418d729423e80fa91d191f72222bc0593154d04a`](https://github.com/sguzman/pulsewire/commit/418d729423e80fa91d191f72222bc0593154d04a).
- Exact tested head: `a1d573c7c1f3885701a8b3bab89908a8483819a7`.
- [Rust, docs and desktop/mobile reader 43790162268](https://github.com/sguzman/pulsewire/actions/runs/43790162268)
  **success**, including full Rust workspace tests, PostgreSQL service/due/watch
  regressions, scheduler backlog, Clippy, source/documentation links and
  a new no-npm PW-032 reporter test.
- [Container packaging and smoke 43790162231](https://github.com/sguzman/pulsewire/actions/runs/43790162231)
  **success**, including non-root slim image checks, disposable
  PostgreSQL/SQLite feed→API/TUI/reader workflows, prior PW-023
  file-backed secrets, authenticated Prometheus and private TLS ingress.
- The docs CI log explicitly states:
  `PW-032 synthetic resource envelope passed: CPU, memory, DB growth, backlog, bounded HTTP p95, provenance and fail-closed inputs`.

## Synthetic math and input contracts

[Fixture](../../tests/fixtures/pw032-resource-synthetic.json):
fictional slim catalog (10 news, 5 government, 5 journals);
four samples over 180 seconds.

| Test field | Synthetic fixture result |
|---|---:|
| Container CPU sampled maximum | 40% |
| Container CPU sampled mean | 23.75% |
| Memory sampled maximum | 150,000,000 bytes |
| Isolated example database size change | +3,000,000 bytes |
| Observed due backlog change | -23 |
| Completed action counter change | +50 |
| Aggregate HTTP latency histogram observation count | 50 |
| Approximate p95 HTTP latency finite bucket upper bound | 500 ms |

These values are **invented solely for regression arithmetic**;
they are not CI runner measurements, real feed benchmarks or host
capacity estimates.

The test harness also rejects invalid provenance/source type, missing
or unexpected fields, illegal catalog categories, negative/fractional
or nonfinite fields, too few/unsorted samples, counter resets,
cumulative histogram inversion and inconsistent interval histogram
deltas. It verifies cases with no new latency observations,
unbounded +Inf p95, rising backlog warning, and an operator-supplied
source claim that still yields `target_host_verified: false`.

The reporter never retrieves secrets or contacts Docker, databases,
networks or APIs. It reads only its explicitly supplied file/stdin,
rejects extra input keys, and emits derived numeric aggregates and
a machine-readable **no-credit** acceptance object.

## Explicit limitations / remaining acceptance

The reporter's sampled CPU % is not a complete host saturation
measurement; its sampled p95 memory is not a sustained resident
memory bound. The input's database-size numbers must represent
Pulsewire-owned data rather than a shared PostgreSQL database.
True total due backlog cannot be inferred from
`pulsewire_due_feeds` last-observed scheduler gauge, and histogram
p95 gives only an upper bucket, not exact elapsed HTTP latency.
Synthetic time windows say nothing about external feed/network
behavior, storage retention, other household workload contention
or performance sustainability.

[PW-032 issue #62](https://github.com/sguzman/pulsewire/issues/62)
remains open pending owner-approved target-host measurements and
sustainable budget evidence. PW-023 security and PW-071
owner-approved deployment/release remain separate. **No
household infrastructure, real credentials/feed data or
release tags were modified.**
