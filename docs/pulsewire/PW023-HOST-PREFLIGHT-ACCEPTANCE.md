# PW-023 — read-only network preflight isolated acceptance

**Verified in disposable CI on 2026-11-15; not real-host acceptance.**
PW-023 remains active **0/3**. Canonical weighted project score
remains **93/100 (Δ +0)**.

## Exact source and green gates

- [PR #60](https://github.com/sguzman/pulsewire/pull/60)
  squash-merged as [`45d7d6793ec04ab9750887c6c13c5dae80a3bacc`](https://github.com/sguzman/pulsewire/commit/45d7d6793ec04ab9750887c6c13c5dae80a3bacc).
- Exact tested head `0afc0a20d316542201a0c93c6c2239d3c94848fe`.
- [Rust, docs and Chromium 43777061823](https://github.com/sguzman/pulsewire/actions/runs/43777061823):
  completed **success** including formatting, workspace compilation/tests,
  PostgreSQL watch/scheduler checks, Clippy and browser/documentation contracts.
- [Disposable container run 43777061828](https://github.com/sguzman/pulsewire/actions/runs/43777061828):
  completed **success** including non-root build, PostgreSQL/SQLite,
  reader/TUI, file-backed DB secrets, authenticated metrics, TLS ingress,
  and the new offline and real-Docker preflight assertions.

The latter log explicitly includes:

```text
PW-023 read-only inventory: all synthetic allow/deny/review cases passed
PW-023 live read-only inventory passed: Docker metadata sanitized, static port/network checks pass, DB trust requires review
```

## What was verified

| Assertion | How verified |
|---|---|
| Positive preflight requires explicit trust | Synthetic ingress + monitoring snapshot with all reviewed database-network members returns `PASS_STATIC` (exit 0), always `deployment_approved:false` |
| Absent or incomplete allowlist cannot pass | The same fixture without a list or with a missing DB member returns `NEEDS_REVIEW` (exit 2) |
| Port exposure and bridge membership detected | Synthetic direct 8091 publication, public 0.0.0.0 TLS binding, proxy attached to DB network, unexpected internal-app member, public Prometheus port and noninternal monitoring bridge return `FAIL` (exit 1) |
| Invalid or stopped state detected | Incomplete schema, unreadable/invalid snapshot and stopped reader are differentiated from valid static approval |
| Narrow Docker read-only collection | The collector invokes only selected `docker inspect --type container --format` and `docker network inspect --format` fields, excluding environment, mounts, labels, logs and secrets; no write or network-creation operations |
| Live isolated Docker topology | The running CI TLS proxy, private app bridge and reader were inspected without mutating them; static topology checks were true, **but** without manually reviewed DB members the live report **correctly** returned `NEEDS_REVIEW` |
| No automatic host/release approval | Every report emits `deployment_approved: false` and explicit unknowns for firewall, routing, TLS, owner privileges and metrics storage/access |

## Unverified, not approved

The tests did **not** inspect the owner's machine, the household
PostgreSQL network, Docker daemon privileges, actual firewall/IPv6
routing, router/NAT, trusted TLS certificate rotation or public
proxy hops. The optional monitoring profile's named-volume
permissions and retention were not audited on a real host.
Even a manually allowlisted DB-network member can bypass
the ingress proxy; an automated network inventory cannot prove
whether all allowed members are trustworthy.

**No household Docker network, containers, CoreDNS, router, firewall,
running service, real database/feed, credential, public port or
release/tag was touched.** An owner-approved real-host evidence review
and separate active-path testing are still required for PW-023.
PW-032 resource measurements and PW-071 release acceptance remain
independent.
