# Pulsewire — troubleshooting and failure isolation

**Read-only diagnostics first.** These procedures identify issues in an
already approved or disposable deployment; they do not authorize starting
Pulsewire on the household host, modifying a shared Docker network,
opening firewall ports, running migrations against real data, or
resetting/removing databases. Do not paste secrets, bearer tokens, full
container environments, SQL dumps, private RSS URLs, or cookies into logs.

See [DEPLOYMENT.md](DEPLOYMENT.md) for preflight,
[COMPATIBILITY.md](COMPATIBILITY.md) for supported/unknown combinations,
[MONITORING.md](MONITORING.md) for health semantics, and
[BACKUP-RESTORE.md](BACKUP-RESTORE.md) for **copy-only** restoration drills.

## Start with these non-mutating observations

From the **repository root**, with the selected Docker network and database
environment supplied **privately**, inspect Compose validity. No container
is started by the `config --quiet` command:

```sh
docker compose -f crates/fetcher/docker-compose.yml config --quiet
docker compose -f crates/server/docker-compose.yml config --quiet
```

If an **already authorized** reader is running on the loopback port, inspect
liveness and readiness independently:

```sh
curl -fsS http://127.0.0.1:8091/health
curl -fsS http://127.0.0.1:8091/ready
```

A successful `/health` does **not** prove SQL connectivity, and a
successful `/ready` does **not** prove that the separate fetcher scheduler
is alive. Do not expose these diagnostics publicly.

## Symptoms and grounded checks

| Symptom | What to inspect safely | What it means / next gate |
|---|---|---|
| Compose refuses to parse | Check whether `PULSEWIRE_DOCKER_NETWORK` and, for the secrets overlay, `PULSEWIRE_POSTGRES_PASSWORD_SOURCE` are provided **without printing values**; compare the selected Compose files | The network must pre-exist; the optional secret-file source must exist and be readable. Default Compose now allows an empty direct-password env only so file overlays can parse; the application refuses an empty effective PostgreSQL credential |
| PostgreSQL rejects startup credential | Check the configured direct env/TOML password **or** `PULSEWIRE_POSTGRES_PASSWORD_FILE`, never both | File secret is an explicit regular UTF-8 file and must be readable inside the container; errors fail closed, with no automatic insecure fallback. See [SECRETS.md](SECRETS.md) |
| Reader returns connection refused | Check `/health` on the **approved local endpoint**, then inspect whether the server is running and which config was loaded | Default host-published listener is loopback 8091; raw `0.0.0.0` exists only **inside** the container. Do not publish another port as a quick fix |
| `/health` returns 200, `/ready` returns 503 | Verify database reachability/permissions from the service network and that fetcher schemas are initialized on the selected database | `/ready` runs a bounded SQL `SELECT 1`; it cannot prove the fetcher scheduler is healthy |
| Browser shows a login error / API 401 | Confirm username and operator-created account; verify the correct server URL and that a token has not expired or been revoked | There is no self-registration and no public reset. An account operator can follow [RECOVERY.md](RECOVERY.md) offline; don't bypass token validation |
| Login returns 429 | Review the documented five-attempt username and 60-attempt TCP-peer quotas per 15 minutes; a shared NAT/proxy may aggregate multiple clients | `Retry-After: 900` is expected on blocked requests. Never trust arbitrary forwarded-IP headers to bypass throttling |
| Feed list contains no stories | Distinguish unsubscribed/all-ingested views, a newly empty database, disabled catalogs, ingestion failures and a paused fetcher | The API's global feed-count and subscription unread-count endpoints have **different scopes**; see [PW061 acceptance](PW061-ACCEPTANCE.md). An empty subscription is not proof of failed ingestion |
| Some feeds never update | Check the configured category, explicit watch enable state, per-domain concurrency, last poll/error backoff and source URL/redirect behavior | YouTube and podcasts are disabled by default. Deleting a watch declaration is **not** equivalent to explicitly disabling a previously ingested watch |
| Public feed blocked as unsafe | Compare the canonical feed host and any redirects with the documented SSRF policy; review DNS/HTTP policy errors privately | Cross-host redirects and private/mixed DNS answers are rejected. Do **not** set `PULSEWIRE_ALLOW_PRIVATE_FEEDS=1` for an untrusted source |
| No Prometheus data | Verify whether metrics were explicitly enabled and reachable **inside an approved private network** | Metrics are disabled by default; no host metrics port is published. `pulsewire_up` is not a guarantee that feeds are fresh |
| Source and server appear to use different articles | Compare both services' selected database dialect, filename/database and the fetcher/server schema settings | With SQLite they must access a compatible **same** database file. With PostgreSQL they must use the same database, with appropriate distinct schemas |
| Restart or migration behaves unexpectedly | Stop any proposed write action and verify the exact commit/config/schema/version and recovery material | Rehearse only against a disposable restored copy. No generalized downgrade or real-data rollback has been accepted |
| CLI refuses `clean` | Confirm this is **not** a production/household database | This destructive command is deliberately blocked without `--confirm`, SQLite + dev mode, `PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET=YES`, and a dedicated existing `PULSEWIRE_DEV_DATA_DIR` |

## Safe error handling

- Capture only **sanitized** HTTP status, local endpoint path, time, build
  SHA and whether `/health`/`/ready` succeeded. A private maintainer can
  examine redacted/limited server logs without posting their complete contents.
- 400 on invalid `read` values and 401 on missing/expired bearer tokens are
  intentional API responses. `next_offset = null` means a page is terminal;
  `next_cursor` is an ingestion-ID high-watermark, **not** the next older
  page token.
- 503 readiness is a database availability signal; investigate connectivity
  and correct config before any mutation. A recovery attempt must not overwrite
  the original database to make a read probe pass.
- If checks require changes to the household router, CoreDNS, Docker network,
  firewall, database schema, storage or server processes, **stop and request
  explicit operator approval**. Do not treat these runbook examples as a
  change ticket.

For a tested clean-room rehearsal, follow [BACKUP-RESTORE.md](BACKUP-RESTORE.md)
using a newly created isolated destination and independently verify articles,
accounts, folders, favorites and read state. Release authorization is a
separate gate under [ROADMAP.md](ROADMAP.md).
