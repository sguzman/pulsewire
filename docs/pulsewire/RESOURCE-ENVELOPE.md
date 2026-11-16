# PW-032 — resource-envelope measurement preparation

**Status: source-only analysis and synthetic CI fixture; the PW-032 target-host
resource envelope is not accepted.** PW-032 remains **0/2 weighted points**
and the canonical Pulsewire project remains **93/100 (Δ +0)**. This document
does not authorize deployment, new feeds, a real database query, a real
Docker measurement, or changes to home networking and services.

## Existing verified evidence and its limits

[PW-031 scheduling acceptance](SCHEDULER_ACCEPTANCE.md) proved bounded
3,225-feed synthetic processing and independent category progress. That test
runs a fake in-process HTTP transport with 1 ms artificial response delay;
it is **not** CPU usage, memory usage, actual PostgreSQL growth, or a
production/server capacity test.

The checked-in [fetcher Compose file](../../crates/fetcher/docker-compose.yml)
currently proposes **0.75 CPUs**, **512 MiB memory**, and **256 PIDs**,
but a declared resource limit is **not** an observed sustainable envelope.
The [default config](../../crates/fetcher/res/docker.toml) polls at
1800 seconds with jitter and allows two global concurrent requests.
The target workload is the **slim news, government, and journals catalog**;
neither the full archived source tree nor videos/podcasts may be counted
as an approved PW-032 load.

## Offline, reproducible analysis

[Resource reporter](../../scripts/pw032-envelope.mjs) accepts only a
strict, sanitized JSON schema with:

- `source` either `synthetic_ci` or **unverified claim**
  `operator_recorded`, and an input-provided 40-hex commit SHA;
- a category count for each of `news`, `government`, `journals`;
- **at least three chronological samples** with CPU percentage
  (container perspective), measured resident/container memory bytes,
  isolated application database size bytes, actual due backlog count,
  and cumulative completed HEAD/GET feed actions;
- `http_latency_buckets`: ten **cumulative counts** for the
  same aggregate HEAD/GET histogram series, with finite upper
  boundaries 25/50/100/250/500/1000/2000/5000/10000 ms and `+Inf`.

For all measurements the reporter computes the sample-window length,
CPU mean/p95/max, memory sampled p95/max, first/last database size
and net change, backlog peak and change, completed action rate,
and a **p95 upper-bound bucket estimate** from histogram-counter
*differences*. This estimate is **not** an exact percentile:
`+Inf` observations without a finite upper-bound return `null`.
It checks strictly increasing timestamps, finite nonnegative
values, monotone counters, cumulative histograms and a fixed input
schema. Unknown fields are rejected rather than echoed: do not
include container environments, secret values, full URLs, host
identifiers, tokens or application records.

To exercise the checked-in **fictional** fixture, not the home host:

```sh
node scripts/pw032-envelope.mjs \
  --input tests/fixtures/pw032-resource-synthetic.json
node tests/pw032-envelope-check.mjs
```

No npm, database, Docker access or network is needed. The analysis
reads one explicitly chosen file (or stdin with `--input -`) and
prints a derived report to stdout; it does not modify any file or
spawn services. **Every report** marks
`pw032_weighted_points: 0`,
`target_host_verified: false`,
`sustainable_capacity_proven: false`, and
`deployment_approved: false` even if the input labels itself
`operator_recorded`. That input label is not an attestation.

## Measurement definitions and important caveats

| Field | Source required in a future approved experiment | Caveat |
|---|---|---|
| CPU % | Repeated fetcher container CPU percent samples, e.g. Docker stats on an authorized isolated host | Multi-core/container CPU percent can exceed 100; sampled p95 is not host-wide saturation |
| Memory bytes | Same container's measured memory usage at each timestamp | Clarify whether the collector reports RSS, working set, or accounting-adjusted usage; no swapping/paging conclusion without additional observations |
| Database bytes | Repeated size of **Pulsewire-owned** PostgreSQL data, under the same accounting definition | `pg_database_size(current_database())` on a **shared** database also includes unrelated services and is **not** a valid Pulsewire-only growth measure |
| Due backlog | Actual pending/due count under a documented time/query definition | `pulsewire_due_feeds` is a last-observed per-category scheduler gauge, **not** necessarily the entire database due queue |
| Completed actions | Sum of HEAD/GET success/error action counters from the same process generation | Restart/counter reset invalidates an unsegmented window; the reporter rejects counter decreases |
| HTTP poll latency | Sum of per-domain/action `pulsewire_http_latency_ms_bucket` cumulative counts across fixed label sets at each sample | Histogram bucket p95 is only an upper bound; include all active action/domain series and no process resets |

Never paste raw Docker inspect output or real SQL credentials into a
fixture. The [metrics security policy](METRICS-SECURITY.md) requires
a private bearer-token file for non-loopback metrics; a real operator
must separately review confidentiality of even the aggregate series.
No real DB query or filesystem inventory is executed by this
reporter.

## Verified synthetic CI (2026-11-15)

[PR #61](https://github.com/sguzman/pulsewire/pull/61) was
squash-merged as [`418d729`](https://github.com/sguzman/pulsewire/commit/418d729423e80fa91d191f72222bc0593154d04a).
The exact checked head `a1d573c7c1f3885701a8b3bab89908a8483819a7`
passed [Rust/docs/reader run 43790162268](https://github.com/sguzman/pulsewire/actions/runs/43790162268)
and [container run 43790162231](https://github.com/sguzman/pulsewire/actions/runs/43790162231).
The docs job log explicitly contains
`PW-032 synthetic resource envelope passed: CPU, memory, DB growth,
backlog, bounded HTTP p95, provenance and fail-closed inputs`.
This is **only** a reproducible test of the analysis math and
input-safety contract, not target-host measurement acceptance.
See [PW032-REPORTER-ACCEPTANCE.md](PW032-REPORTER-ACCEPTANCE.md).
Actual-host evidence remains open in
[PW-032 issue #62](https://github.com/sguzman/pulsewire/issues/62).

## Real disposable CI sampling, still not target-host acceptance

The optional [CI-only observer](../../tests/pw032-disposable-live.sh)
measures a **running test fetcher** against an ephemeral PostgreSQL
database and nginx-served RSS fixture. It collects precisely three
timestamped samples from a known `pulsewire-ci-metrics` container
using Docker stats, a read-only `fetcher` schema SQL query, and the
existing authenticated, non-published `/metrics` listener. Each
raw sample stays in GitHub Actions temporary storage and is discarded.
The [normalizer](../../scripts/pw032-ci-normalize.mjs) extracts
CPU percentage, Docker-reported memory usage, aggregate HEAD/GET
action counters and HTTP histogram buckets, dropping container
identifiers and domain labels. It requires exactly one explicit
unlabeled `pulsewire_up 1` running-state gauge; a misleading
numeric prefix (`pulsewire_up 10`), a comment, a down gauge, a
duplicate or a labeled impersonation cannot establish that the
sampled exporter was running. The normalizer itself has no Docker,
network, SQL or credential access, and its
[offline test](../../tests/pw032-ci-normalize-check.mjs) exercises
actual Docker-stats and Prometheus wire formats with fake values,
including negative running-gauge cases.

Database bytes use `pg_total_relation_size` for ordinary and
materialized tables in the `fetcher` schema only, including
their indexes/TOAST; this is **not** WAL, all schema allocation,
free-space accounting or shared database size. Due backlog uses
`fetcher.feed_state_current.next_action_at <= CURRENT_TIMESTAMP`
against a disposable DB. The test fixture contains a **single
news feed**, not a representative live slim catalog. It may not
generate enough HTTP actions within the brief sample window to
produce a meaningful latency p95, and it cannot measure sustained
capacity or rate under real network behavior.

The shell script refuses to run outside GitHub Actions without the
explicit `PULSEWIRE_PW032_DISPOSABLE=YES` guard, requires exactly
the fixed named CI containers and isolated network, and checks no
metrics port is host-published. It does **not** mutate containers,
feed data, SQL tables, networks or services. It prints only a small
derived aggregate without bearer tokens, SQL results, raw metrics,
Docker identifiers or source URLs. In a future approved real-host
experiment, a different collector design and explicit owner
authorization would be needed; this CI-only script must not be
repointed at household Docker or PostgreSQL.

**Even when live CI passes, this remains PW-032 pre-acceptance only:
0/2 weighted points, and the project stays 93/100.**

## Exact-head live CI evidence (2026-11-15)

[PR #63](https://github.com/sguzman/pulsewire/pull/63)
was squash-merged as [`a6d7a4c`](https://github.com/sguzman/pulsewire/commit/a6d7a4c852e649e1d155777ce5f7d6d4bc4d329e).
Both [Rust/docs/browser CI 43792770359](https://github.com/sguzman/pulsewire/actions/runs/43792770359)
and [container CI 43792770308](https://github.com/sguzman/pulsewire/actions/runs/43792770308)
passed on exact tested head `57c904fe940b96135c3385ed5a4d3cf9ae07c918`.
The container log confirms **three measured snapshots** of the single
disposable RSS fixture, a **0.18% sampled maximum Docker CPU** and
**3,702,522 bytes sampled maximum Docker-reported memory**. These numbers
are only incidental runner observations; do not use them to forecast
sustained server capacity or mean/p95 real workload costs.

The same CI run passed the existing PostgreSQL, SQLite,
authenticated Prometheus, and private TLS ingress smoke.
See [PW032-DISPOSABLE-ACCEPTANCE.md](PW032-DISPOSABLE-ACCEPTANCE.md)
for the exact test scope and unresolved target-host criteria.
**PW-032 remains 0/2; project remains 93/100 (Δ +0).**

## How PW-032 can be accepted later

A target-host owner-approved experiment needs its exact code commit,
hardware and container limits, catalog inventory/counts, authorized
isolated DB and private metrics access, sample cadence and
wall-clock window, cold/warm start and error/retry behavior. It needs
a representative sustained run including idle and busy periods,
concurrent household service contention, network stalls, and
actual database growth/retention. Collect a **baseline and repeat**
on the target and establish project-specific sustainable CPU,
memory, backlog drain, poll latency and storage budgets. A single
synthetic summary or a momentary Docker stats reading cannot do so.

Use [DEPLOYMENT.md](DEPLOYMENT.md), [MONITORING.md](MONITORING.md),
[BACKUP-RESTORE.md](BACKUP-RESTORE.md),
[PW031-ACCEPTANCE.md](PW031-ACCEPTANCE.md) and
[ROADMAP.md](ROADMAP.md) to review dependencies.
PW-023 actual-host security and PW-071 owner-approved release
remain separate and incomplete. Do **not** change real services,
networking, databases, feed catalogs or release tags merely to
run this synthetic analysis.
