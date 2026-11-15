# PW-023 — authenticated metrics partial acceptance

**Isolated CI subtask verified 2026-11-15. PW-023 remains active 0/3
weighted points; overall Pulsewire remains 93/100 (Δ +0).**

## Exact source and CI

- [PR #59](https://github.com/sguzman/pulsewire/pull/59), squash
  merged at [`5eef609f8e8d6624ce686d64ee8b005c3fb1af3d`](https://github.com/sguzman/pulsewire/commit/5eef609f8e8d6624ce686d64ee8b005c3fb1af3d).
- Exact tested head: `2a42d339d2a977c7c0d704e830f76b43dbcc0756`.
- [Rust/docs/Chromium CI 43770368318](https://github.com/sguzman/pulsewire/actions/runs/43770368318) **success**:
  formatting, full Rust workspace compile and tests (including new
  token/HTTP-body authentication regressions), scheduler metrics,
  PostgreSQL service/watch restart, Clippy, docs-contract checks and
  desktop/mobile browser.
- [Container CI 43770368287](https://github.com/sguzman/pulsewire/actions/runs/43770368287) **success**:
  Compose configuration, non-root images, disposable PostgreSQL/SQLite
  RSS/API/TUI/browser acceptance, independent file-backed PostgreSQL
  secrets and isolated TLS ingress, and the new live metrics acceptance.

The container log contains:

`PW-023 authenticated metrics passed: non-loopback fail-closed, anonymous/wrong-token 401, bearer scrape 200, Prometheus up/down`

## What was actually exercised

| Control | Evidence |
|---|---|
| Default remains off | Checked-in fetcher `[metrics].enabled = false`, loopback default, no host-published port; Compose and docs checks continue to assert both |
| Non-loopback fail-closed | The same real fetcher with a non-loopback metrics bind and **no** token file exits unsuccessfully before scheduling, with a generic guard error |
| Unauthorized network client | A disposable network peer on the database network receives HTTP 401 without credentials and with a wrong bearer token |
| Authenticated scrape | The real fetcher using a mounted token file returns Prometheus samples with `pulsewire_up 1` to the correct bearer; the token value is written to a CI-only file and the fetcher is passed only a **path** in its environment |
| Prometheus integration | Pinned Prometheus reads `bearer_token_file`; its query reports `up=1` while fetcher runs and `up=0` after the fetcher is stopped |
| HTTP parser and auth | Rust unit tests cover token parsing, missing/wrong/duplicate Authorization headers, header/body separation, request framing and loopback backward compatibility |
| Opt-in Compose example | Docs/Compose checks require no published metrics or Prometheus UI ports, an internal monitoring bridge, explicit private token and private config source, and proposed 24h/128MB storage limits |

## Unverified and explicitly not claimed

- No host network, installed Prometheus, real certificate, production
  database, monitoring token or secret store was used.
- The **optional** Prometheus overlay was parsed and source-checked;
  its named-volume ownership, non-root filesystem accessibility and
  lifecycle were **not exercised on the household host**.
- A bearer token protects samples from unauthorized HTTP peers; it does
  not encrypt transport. A token sent in plaintext on an insufficiently
  trusted network may be intercepted. Anyone with Docker-admin access
  or file permissions can bypass container network/secret boundaries.
- The fetcher remains on its PostgreSQL services network; containers
  already there can contact its HTTP socket and attempt authorization.
  The example monitoring network is not proof of least-privilege
  membership, firewall behavior or production Prometheus API security.
- Proposed 24h/128MB TSDB limits are not an audited real retention,
  backup or access-control policy; file-backed secrets do not by
  themselves support token rotation.
- PW-023 actual-host trust boundary/certificate/metrics acceptance,
  PW-032 resource envelope and PW-071 owner-approved deployment
  remain separate outstanding gates.

**No household Docker network, CoreDNS, router, firewall, service,
PostgreSQL dataset, RSS catalog, real secret, public port, release
version or tag was changed.**
