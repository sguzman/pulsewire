# PW-023 — authenticated, non-public metrics

**Status:** opt-in authenticated metrics code and disposable CI profile;
**not** a production monitoring deployment or a completed PW-023 security
milestone. The canonical project remains at **93/100**, and PW-023 at
**0/3 weighted points** pending actual-host acceptance.

## Default and new fail-closed rule

Pulsewire's fetcher has a built-in Prometheus-text HTTP listener.
It is **disabled by default**. The checked-in Docker fetcher config
binds `127.0.0.1:9898` only when deliberately enabled, with no
published host port.

When an operator enables metrics, the listener now enforces these rules:

- If its configured bind address is **not loopback** (including
  `0.0.0.0:9898`), startup **requires**
  `PULSEWIRE_METRICS_TOKEN_FILE`. Without a valid token file, the
  fetcher fails startup instead of serving unauthenticated metrics.
- `PULSEWIRE_METRICS_TOKEN_FILE` is a **path**, not secret bytes
  in the environment. The file must be readable as UTF-8, contain
  32–512 printable ASCII characters with no spaces/control characters,
  and may have one trailing LF or CRLF. Missing, empty, short,
  multiline or unreadable files fail closed with **generic** errors
  that do not echo the path or token. A token file also enables
  authentication on a deliberately loopback-bound listener.
- If metrics are enabled **only on loopback** and no token file
  is configured, local loopback scraping remains unauthenticated
  for backwards compatibility. This does not mean host users,
  other processes sharing the container's network namespace,
  or authorized Docker administrators are untrusted.
- Authenticated requests use
  `Authorization: Bearer <private-token>` on
  `GET /metrics`. Unauthenticated and incorrect/duplicate
  bearer headers get **401**, not metric samples. Unknown paths
  get 404; unsupported request methods do not serve data.
  Authorization headers inside the request **body** are ignored.
  The handler bounds the header size (8 KiB), header-read time
  (3 seconds) and concurrent connections (64).
- The listener does **not terminate TLS**. A bearer token sent
  over plaintext HTTP can be intercepted by someone with access
  to the private network. For real-host deployment, use a
  separately reviewed TLS/transport trust boundary or an
  appropriately private trusted network. A token is not a
  substitute for firewall/network access controls.

This change **intentionally breaks** previously enabled
`0.0.0.0` metrics listeners that had no authentication. Provide
an approved private token file or revert to loopback-only access.
Do not weaken the guard by sharing the token with arbitrary clients.

## Optional monitoring Compose profile

[Fetcher metrics overlay](../../crates/fetcher/docker-compose.metrics.yml)
is **not** enabled automatically. It adds a dedicated internal
`pulsewire_metrics_private` network, a token-file mount for
the Rust fetcher, and a pinned Prometheus scraper attached to
the monitoring network **but not** to the external PostgreSQL
services network. Neither the fetcher metrics port nor the
Prometheus web interface is published on the host.

[Prometheus example configuration](../../deploy/pw023/prometheus.yml)
uses `bearer_token_file` to read the same private token. It
proposes a **24-hour** time retention and **128 MB** TSDB size
cap; the Prometheus data is in a named Docker volume. Those
limits are configuration defaults for the **optional example**,
not evidence of real-host retention/access-control compliance.

To inspect the configuration **without running containers**,
an operator would first supply privately:
`PULSEWIRE_DOCKER_NETWORK` (an existing trusted database network),
`PULSEWIRE_METRICS_CONFIG_SOURCE` (an **absolute path** to a full private fetcher
TOML file, based on the reviewed config, with
`[metrics].enabled = true` and `bind = "0.0.0.0:9898"`),
`PULSEWIRE_METRICS_TOKEN_SOURCE` (a real private token file),
and a separate PostgreSQL password source. Then:

```sh
docker compose -f crates/fetcher/docker-compose.yml \
  -f crates/fetcher/docker-compose.metrics.yml config --quiet
```

A successful parse does **not** prove that token files are
readable by the non-root fetcher UID 65532, that the selected
network is isolated, that Prometheus can write its TSDB,
that Docker has correct volume permissions, or that metrics
are secure on a real host. The source-file secret is **not**
automatically a managed secret store, rotation mechanism
or encryption solution. The example does not include an
automated metrics-token rotation protocol.

**Important network limit:** the fetcher remains connected to
its **database network** as well as the private monitoring
network. A container on the database network may still make
HTTP requests to `9898` when metrics is bound to all
interfaces, but the Rust endpoint rejects requests without
the bearer token. Privileged Docker administrators can
always read mounted secrets, join networks and bypass
container isolation. Prometheus itself has an unauthenticated
web/API listener on the private monitoring network; **do not**
attach untrusted containers or publish that interface.

## Isolated CI checks and accepted boundaries

Existing CI verifies a real, pinned Prometheus scraping a
disposable PostgreSQL-backed fetcher and detecting its
subsequent shutdown. The new PW-023 checks extend this:

1. A non-loopback listener **refuses startup** with no token file.
2. The same database-network peer gets **401** on anonymous
   and incorrect-token HTTP requests; there is no public
   host-published metrics port.
3. A correct file-backed bearer token produces HTTP 200
   and `pulsewire_up 1`, without putting the metrics
   token bytes into the fetcher container environment.
4. Prometheus reads the mounted token through
   `bearer_token_file` and observes `up=1`, then
   `up=0` after the fetcher is stopped.
5. Rust unit tests validate token-file parsing, loopback
   compatibility, rejection of wrong/duplicate/missing
   tokens and body-header spoofing.
6. Compose/source checks reject changes that accidentally
   add published metrics or Prometheus ports, omit the
   private monitoring network, or remove the token source.

These checks are **disposable CI evidence only**, not an
approved real-host firewall/port scan, credential-rotation
test, monitoring retention audit or production TLS proof.

## Still required before PW-023 closure

Confirm all host Docker-network memberships, firewall/ingress
routes, TLS and client-IP trust in the actual network,
file permissions and rotation plan, Prometheus UI and TSDB
access restrictions, retention/backup policy, and actual
host-specific behavior. PW-032 resource constraints and
PW-071 release/deployment acceptance are separate.
Never modify the household PostgreSQL, router, CoreDNS,
services, Docker network, secrets or firewall merely to
run this guide. See [MONITORING.md](MONITORING.md),
[INGRESS.md](INGRESS.md), [SECRETS.md](SECRETS.md)
and [ROADMAP.md](ROADMAP.md).
