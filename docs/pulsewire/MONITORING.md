# Pulsewire monitoring and readiness

Pulsewire runs its HTTP reader server and RSS fetcher as separate processes. Their health signals therefore cover **different dependencies**.

## Reader server

- `GET /health` is a liveness probe: HTTP 200 with `ok` whenever the HTTP server is running. It does not query storage.
- `GET /ready` is a readiness probe: HTTP 200 with `ready` only when the configured SQLite or PostgreSQL pool can execute `SELECT 1` within two seconds. A missing, closed, unavailable or timed-out database returns HTTP 503 with `not ready`.
- Neither route returns connection strings, credentials, or detailed database errors.
- The server does **not** own the RSS scheduler. A successful `/ready` means the reader API has database connectivity, not that the fetcher is processing feeds.

For a deliberately approved loopback deployment:

```sh
curl -fsS http://127.0.0.1:8091/health
curl -fsS http://127.0.0.1:8091/ready
```

The container workflow checks `/ready` against a disposable PostgreSQL-backed server; Rust tests check SQLite success, missing database state and closed-pool failure.

## Fetcher metrics

The fetcher's Prometheus-text endpoint is **disabled by default**. Its example Docker config sets `[metrics].enabled = false` and `bind = "127.0.0.1:9898"`; the fetcher Compose service publishes no metrics port. A monitoring operator must explicitly enable a private, reviewed scrape path. When the fetcher binds metrics to **any non-loopback interface**, startup now **requires** a private file-backed bearer token; anonymous scrapes are rejected. Loopback-only metrics without a token remain supported for local development. See [authenticated metrics and network policy](METRICS-SECURITY.md). Do not bind or publish the endpoint on an untrusted network.

When enabled, `GET /metrics` exposes `pulsewire_up`, start time, scheduler ticks, due-feed gauges and counters, HTTP action outcomes and latency, database query timing, and in-flight actions. The core's isolated loopback regression tests check a parseable HTTP response, proper content length, and Prometheus sample framing.

**CI-verified real Prometheus scrape:** [PR #46](https://github.com/sguzman/pulsewire/pull/46) runs a second metrics-enabled fetcher and a pinned Prometheus container on a disposable, private test network without host port publishing. Its [exact-head container run 43745925819](https://github.com/sguzman/pulsewire/actions/runs/43745925819) checks `up{job="pulsewire-fetcher"} == 1` while the fetcher runs and `up == 0` after it stops. This proves real scrape acceptance and process-loss visibility in CI, **not** that monitoring or alerts have been installed on the owner's home host. Production metrics remain disabled by default.

The optional [authenticated metrics Compose overlay](METRICS-SECURITY.md)
attaches Prometheus to an internal monitoring bridge, with no host-published
Prometheus port and a bearer-token-file scrape. It does not restrict the
fetcher's existing database-network attachment, and a stolen token or
compromised trusted container could still access metrics. Its proposed
24-hour / 128 MB retention is **not** a verified production retention
or storage-permissions policy.

Metrics indicate activity, **not** comprehensive data freshness. A scrape failure or a missing target should be treated as a fetcher outage; `pulsewire_up 1` is only emitted while the endpoint responds. Configure Prometheus target `up` alerts and separately monitor reader `/ready` failures, fetcher process exits, feed backlog and scrape staleness.

## Ingress distinction

The optional [TLS ingress profile](INGRESS.md) deliberately rejects
external `/metrics` requests, and the fetcher remains disabled/unpublished
by default. This edge behavior does **not** prove that an independently
enabled metrics target is inaccessible from other Docker networks, or
supply any production metric-authentication/retention policy. Real-host
scrape permissions and network membership are separate PW-023 gates.

## Exposure and release boundary

The open [PW-023 security gate](https://github.com/sguzman/pulsewire/issues/9) still requires trust boundaries, firewall, TLS/proxy, credentials and metrics network access to be verified before a home deployment. These probes do **not** authorize opening firewall ports, exposing API/metrics to the internet, changing household services, or enabling monitoring against production data.
