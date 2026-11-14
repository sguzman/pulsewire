# PW-031 — Fair bounded scheduling acceptance

Acceptance scope: roadmap M3 / PW-031 (5 weighted points). This record documents
**isolated CI evidence**, not operational deployment authorization.

## Evidence matrix

| Criterion | Verified implementation | Evidence |
|---|---|---|
| Independent category tick and retry loops; blocked and failing due queries do not hold back other categories | [PR #12](https://github.com/sguzman/pulsewire/pull/12), [PR #13](https://github.com/sguzman/pulsewire/pull/13) | Rust and container CI passed |
| Global/domain request limits and no global slot held while waiting on a busy domain | PR #12 | Shared permit regression and aggregate workload |
| Slow HTTP feed cannot starve a healthy category | [PR #16](https://github.com/sguzman/pulsewire/pull/16) | Stalled GET regression; both CI pipelines passed |
| Failed HTTP action backs off while another category progresses | [PR #18](https://github.com/sguzman/pulsewire/pull/18) | Synthetic HTTP timeout with persisted error/healthy state; both pipelines passed |
| Exponential retry deadlines, maximum backoff, timely recovery | [PR #20](https://github.com/sguzman/pulsewire/pull/20) | Link-state tests; both pipelines passed |
| Stable due ordering, >1,000-row paging, deferred/error exclusion in both SQL backends | [PR #15](https://github.com/sguzman/pulsewire/pull/15), [PR #17](https://github.com/sguzman/pulsewire/pull/17), [PR #21](https://github.com/sguzman/pulsewire/pull/21) | In-memory SQLite and disposable PostgreSQL TEMP table regression tests; both pipelines passed |
| Bounded repeatable multi-category backlog with no duplicate work | [PR #19](https://github.com/sguzman/pulsewire/pull/19) | Bounded three-round synthetic load; both pipelines passed on rebased head |

## Synthetic workload measurement

Final rebased PR #19 [Rust run 43609054897](https://github.com/sguzman/pulsewire/actions/runs/43609054897) logged:
**3,225 synthetic feeds**, 3 categories, 3 rounds, **2,291 ms elapsed**,
**3 peak in-flight HTTP actions** against a configured global budget of 4,
zero in-flight at completion, and no repeated feed processing.
[Container run 43609054901](https://github.com/sguzman/pulsewire/actions/runs/43609054901)
passed including disposable PostgreSQL HTTP smoke.

These figures are from an offline fake HTTP/Repo test on GitHub Actions.
They are **not** real-world RSS network throughput, host CPU/memory profiling,
or evidence of sustainable performance on the intended Linux installation.

## Final integrated merge gate

This documentation PR starts from combined main after PRs #17 through #21
merged. Its full Rust and container CI must pass on the combined source head
before the roadmap marks PW-031 complete. This includes PostgreSQL runtime
and due-queue acceptance, scheduler backlog test, Clippy, container builds
and disposable end-to-end HTTP smoke.

## Deliberate scope boundaries

Actual CPU, RSS, DB growth, real polling latency and long-running resource
envelope remain **PW-032**, pending authorized target-host testing.
Real deployment and network ingress remain blocked under **PW-023/PW-043**
and later release criteria. No home service or database is changed by CI.
