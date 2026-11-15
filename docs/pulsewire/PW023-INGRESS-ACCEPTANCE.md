# PW-023 — isolated ingress and network acceptance evidence

**Verified subtask only, 2026-11-15; PW-023 overall remains active 0/3.**
Canonical Pulsewire weighted acceptance remains **93/100 (Δ +0)**.

## Source and exact-head gates

- [PR #58](https://github.com/sguzman/pulsewire/pull/58) merged as
  [`22e90ee4eb0fb9c66f3d7247ab7cd613ad061c7d`](https://github.com/sguzman/pulsewire/commit/22e90ee4eb0fb9c66f3d7247ab7cd613ad061c7d).
- Exact checked PR head: `7001d89b4df79eefe3ba5ef1b1f03ef6a79602b7`.
- [Rust, docs and browser 43767148246](https://github.com/sguzman/pulsewire/actions/runs/43767148246):
  **success**, including full Rust tests, scheduler/PostgreSQL integration,
  Clippy, docs/ingress source guardrails and desktop/mobile Chromium.
- [Container packaging and live smoke 43767148289](https://github.com/sguzman/pulsewire/actions/runs/43767148289):
  **success**, including prior PostgreSQL/SQLite API/TUI/reader/backup cases
  and a new isolated real-server ingress test. The job log concludes:
  `PW-023 TLS ingress passed: verified HTTPS, minimal routes,
  distinct-peer login limiting, forged-header resistance, and
  edge-to-backend network isolation`.

## Verified disposable properties

| Requirement | Observed in executable CI |
|---|---|
| Host-published port boundary | Optional Compose overlay replaces `server.ports` with an empty list, checked against merged JSON; NGINX publishes only `127.0.0.1:8443` (18443 fixture port) |
| Separate data/front/app networks | Real Docker inspect proves proxy is on private-app and edge networks but **not** DB network; server is on private-app and DB networks but **not** edge |
| HTTPS protocol and identity | Temporary self-signed certificate with explicit DNS and IP SAN; verified using curl CA trust, with TLS 1.2/1.3 configuration; plaintext HTTP not accepted on the proxy port |
| Minimal ingress paths | `/reader` loads; `/v1/entries` denies missing bearer (401); `/docs`, `/openapi.json`, `/health`, `/ready`, `/metrics` and unknown routes return 404 |
| No direct front-to-back access | Untrusted edge client cannot reach internal reader application-bridge IP or hostname over HTTP; reader has **no** directly published host port |
| Real-peer throttling | Two persistent edge clients have different true Docker IPs; spoofed `X-Forwarded-For`, `X-Real-IP` and `Forwarded` on repeated login requests still lead to edge HTTP 429 for client A; independent client B gets a normal 401 |
| Metrics boundary | Metrics are not routed by this TLS proxy and remain disabled/unpublished in default fetcher config; **not** a proof of production Prometheus isolation |

## What is not proven

This CI-only topology does **not** cover actual household Docker network
membership, DB-network-to-reader bypass, firewalls from any non-CI host,
production certificate issuance/renewal/revocation, the real reverse proxy
chain, an approved public listener, external source-IP trust policies,
production metrics credentials/retention, or owner-approved service
deployment. Any container permitted on the same database network as the
reader may bypass the proxy unless the operator independently restricts
that trust domain. Docker administration/root can always change networks
and access host secrets.

**No household database, router, DNS, running services, firewall, network,
real credentials, source feeds, production metrics, public port, release
version or tag was modified.** PW-023 is still a P0 open issue; PW-032 and
PW-071 remain separate resource/deployment gates.
