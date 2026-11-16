# PW-071 — release gate source-only acceptance (partial)

**Verified 2026-11-15 in source and isolated CI.**
This is **not** PW-071 release/home-host acceptance.
PW-071 remains **pending 0/2**. The canonical project is
**93/100 (Δ +0)**. No release, version bump, git tag or publication.

## Exact checked source and CI

- [PR #64](https://github.com/sguzman/pulsewire/pull/64)
  squash merged at [`d8b88b7e6968ea016db806943dbe746769e0d4e4`](https://github.com/sguzman/pulsewire/commit/d8b88b7e6968ea016db806943dbe746769e0d4e4).
- Exact tested head: `fa896efc1d2ff261e64f61ff1c0c1ab5210d1f4b`.
- [Rust/docs/reader run 43795288832](https://github.com/sguzman/pulsewire/actions/runs/43795288832):
  completed **success**, including Rust formatting, workspace
  compile/tests, PostgreSQL integration, scheduler due and watcher
  restart, Clippy, desktop/mobile Chromium reader, document links
  and new PW-071 release-preflight negative tests.
- [Container run 43795288869](https://github.com/sguzman/pulsewire/actions/runs/43795288869):
  completed **success**, including non-root image builds, disposable
  PostgreSQL/SQLite feed→reader/TUI, private authenticated Prometheus,
  read-only network inventory, isolated HTTPS ingress and real
  short-window PW-032 CI observations.
- The docs job explicitly reports:
  `PW-071 release preflight passed: canonical 93/100 blockers, fail-closed legacy recipes, synthetic approval validation, no tag/push`.

## Verified release-safety properties

| Property | Observable source/CI evidence |
|---|---|
| Canonical milestone state | Strict parser finds 27 frozen P0/P1 milestones, total weight 100, accepted weight 93, exact pending PW-023 (3), PW-032 (2) and PW-071 (2) |
| Reconciliation failures | Duplicated/missing rows, unknown statuses and mismatched stated score fail closed |
| No automatic release shortcuts | Root `just major`, `just minor` and `just patch` recipes no longer call `cargo release --execute`, tag, push, publish, deploy or bump versions; a readiness gate denies current unfinished milestones |
| Safe status command | `node scripts/pw071-release-preflight.mjs --status` or `just release-status` only reads the canonical roadmap and emits JSON; a successful status read is not approval |
| Private owner receipt schema | Future declaration requires exact repository, HEAD SHA, release type, UTC timestamp, all six separately claimed gate approvals, references, clean `main` and an owner-private file **outside** the repo; tests reject missing/wrong/extra fields |
| No invented acceptance | Every status report has `release_approved:false` and `external_host_evidence_verified:false`; receipt validation explicitly states it is **not** cryptographic approval or remote CI verification |
| Exact-head CI controls | Both workflows succeeded for the same head before merging. No version or image was published to provide this proof |

## Outstanding prerequisites

[PW-023 #9](https://github.com/sguzman/pulsewire/issues/9)
still needs real-host ingress/firewall/TLS/secret trust acceptance.
[PW-032 #62](https://github.com/sguzman/pulsewire/issues/62)
still needs representative sustained target-host CPU/memory/DB/latency
and backlog measurements.
[PW-071 #65](https://github.com/sguzman/pulsewire/issues/65)
still needs **all** P0/P1 gates closed, exact release-commit
evidence, tested target-host deployment, verified actual backup
and restore/rollback, and explicit owner approval for the exact
version/tag and any publication.

The receipt parser cannot independently authenticate an owner,
inspect remote CI runs or establish real-host safety. It is an
additional fail-closed safeguard, **not** a substitute for explicit
authorization. The Just recipes deliberately cannot issue any
release operation even with a future accepted receipt.

**No household Docker services, databases, CoreDNS/router,
storage, firewall, credentials, real feeds or public ports were
accessed or modified. No git tag, push, version bump, deployment,
container registry publication or public release was performed.**
