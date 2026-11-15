# PW-050 — Targeted deterministic regression acceptance

**Scope:** Roadmap M5 / PW-050 (4 weighted points). This matrix is
a final **combined-source CI gate**: the source changes are already
merged on `main`, but this milestone is not credited until this
documentation PR passes both full Rust and disposable-container
pipelines at its own final branch head and merges.

## Verified coverage matrix

| Acceptance area | Tests, implementation, and evidence | Scope of the assertion |
|---|---|---|
| Malformed and oversized feeds | `crates/core/src/feed/parser.rs::offline_feed_regressions`, [PR #32](https://github.com/sguzman/pulsewire/pull/32) | Empty/invalid XML rejected; >8 MiB decompressed body refused before parsing; valid offline RSS and Atom normalize titles, GUIDs, summary and timestamps |
| Disabled historical catalogs | `crates/core/src/infra/config/feeds.rs::suppressed_catalog_tests`, [PR #32](https://github.com/sguzman/pulsewire/pull/32) | Malformed TOML in disabled `podcasts` and `youtube` directories is never parsed; active `news` file loads; irrelevant `global.toml` skipped by feed-file collector |
| RSS HTTP change detection | `crates/core/src/domain/link_state.rs::head_validator_regressions`, [PR #33](https://github.com/sguzman/pulsewire/pull/33) | Matching validators avoid redundant GET; changed ETag/Last-Modified and 304→200 trigger GET; **200 without comparable validators triggers GET** rather than indefinitely missing updates |
| Scheduler fairness and bounded requests | `crates/core/src/app/scheduler/concurrency.rs` tests, independent category-loop tests, and [PRs #12](https://github.com/sguzman/pulsewire/pull/12), [#13](https://github.com/sguzman/pulsewire/pull/13), [#19](https://github.com/sguzman/pulsewire/pull/19) | Global/domain limits, blocked-domain fairness and repeatable multi-category due backlog; the full Rust workflow reruns the synthetic backlog regression |
| Error recovery and backoff | `crates/core/src/domain/link_state.rs::scheduler_retry_tests`, [PR #20](https://github.com/sguzman/pulsewire/pull/20) and [PR #18](https://github.com/sguzman/pulsewire/pull/18) | Bounded exponential backoff on timeout and recovery without starving healthy categories |
| Outbound feed security | `crates/core/src/infra/http_policy.rs::tests` | Reject literal private/link-local/loopback hosts, dangerous URL schemes, credentials and cross-host/cross-scheme redirect conditions; network-side DNS rebinding/egress controls remain a deployment obligation |
| Authentication and quotas | `crates/server/src/login_throttle.rs` atomic reservation/expiry/source tests; container HTTP smoke; [PR #11](https://github.com/sguzman/pulsewire/pull/11) | Account/source quotas, fixed-window expiry, spoofed `X-Forwarded-For`/`X-Real-IP`/`Forwarded` headers, valid/invalid login, password rotation and revoked bearer behavior through real HTTP handlers |
| Data-preserving migrations | [PW-043 acceptance](https://github.com/sguzman/pulsewire/blob/main/docs/pulsewire/PW043-ACCEPTANCE.md) | Disposable backup/restore verifies reader state and history. Credited separately to PW-043, **not counted again** under PW-050 |

## Code and CI provenance

- [PR #32](https://github.com/sguzman/pulsewire/pull/32) merged as
  [15e5be8](https://github.com/sguzman/pulsewire/commit/15e5be87f27954b8081e7a36648d8d07d5680125).
  Final [Rust run 43622130727](https://github.com/sguzman/pulsewire/actions/runs/43622130727)
  and [container run 43622130658](https://github.com/sguzman/pulsewire/actions/runs/43622130658)
  both passed.
- [PR #33](https://github.com/sguzman/pulsewire/pull/33) merged as
  [70342ed](https://github.com/sguzman/pulsewire/commit/70342edc8bb4e5ed9b80cbdcab1b5cca384d0018).
  Final [Rust run 43622845222](https://github.com/sguzman/pulsewire/actions/runs/43622845222)
  and [container run 43622845267](https://github.com/sguzman/pulsewire/actions/runs/43622845267)
  both passed.
- The **final combined main** includes both merges. Accept this milestone
  only after **both** pipelines pass on the current documentation branch
  that starts from this combined main, without subsequent code drift.

## Explicit limitations and separate work

**PW-053 HTML-watch hashing is not complete.** In
`crates/core/src/app/scheduler/actions.rs::compute_body_changed`,
ContentHash/ElementHash detection still treats a present hash as a
potential change without a persisted prior body/element hash for
comparison. PW-050's *RSS HEAD validator change detection* does not
substitute for PW-053's full HTML-watch implementation.

**PW-023 ingress/TLS and network-isolation acceptance is still open.**
Offline URL/redirect and real HTTP auth regressions do not prove
public exposure is safe. **PW-051 TUI/SQLite/PostgreSQL operational
end-to-end acceptance is also separate.** No real feed, home network,
CoreDNS, database, or running service was touched by these tests.

**Progress rule:** credit PW-050's four weighted points exactly once,
after integrated CI passes and the evidence PR merges; until then
the verified project score remains 74/100.
