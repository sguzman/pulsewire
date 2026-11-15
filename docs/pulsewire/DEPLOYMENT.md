# Pulsewire — container deployment runbook

This is a **deployment preflight**, not permission to deploy. Do not run Pulsewire
against an existing household database until a backup and a restore test have
been completed (PW-043). Never run migrations or wipe tools against someone
else's PostgreSQL schemas.

## What is built and tested

- CI builds non-root musl fetcher/server images from the workspace using the
  locked dependencies. Disposable PostgreSQL **and SQLite** fixtures verify
  RSS ingestion, schema migrations, authenticated HTTP APIs, read-state,
  subscriptions/favorites/folders, TUI journeys and service restart. The
  browser is tested on desktop/mobile against a real PostgreSQL API fixture.
  These are **CI integration tests**, not human QA against a household host.
  See [PW-051](PW051-ACCEPTANCE.md), [PW-061](PW061-ACCEPTANCE.md) and
  [PW-043](PW043-ACCEPTANCE.md).
- The default fetcher image contains **non-YouTube, non-podcast** feed catalogs
  only. Original source definitions remain in Git. The loader also skips these
  categories when their directories are mounted.
- The server is published only on **127.0.0.1:8091** by Compose. Fetcher
  metrics are **disabled by default** and bind to **127.0.0.1:9898** if enabled
  without overrides. Compose no longer publishes port 9898. To scrape metrics
  from another container, explicitly review and supply a private metrics config
  and restricted network; never expose the metrics endpoint publicly.
  A trusted reverse proxy on the Docker network may access the server internally;
  there is no automatic public proxy.

## Outbound fetch policy

The fetcher now applies conservative source URL and redirect rules,
connection-time public-IP DNS restrictions, disabled ambient proxies and
bounded body streaming. Read [FETCH_SECURITY.md](FETCH_SECURITY.md) before
allowing third-party source definitions or granting unrestricted egress.

Strict mode is the default. The **opt-in** `PULSEWIRE_ALLOW_PRIVATE_FEEDS=1`
permits internal DNS targets and private addresses for explicitly trusted
sources only; do not enable it casually or in a shared/public feed system.
The disposable smoke uses it for its `ci-rss` fixture. The resolver checks
are not a network sandbox: enforce independent container/firewall egress
rules, trusted DNS and proxy controls before real-host deployment.

## Authentication and inbound proxy safety

The server keeps a persistent per-username login throttle (**five requests in
15 minutes**) and a separate quota of **60 login requests per 15 minutes**
from the actual connected TCP peer, shared across attempted usernames. Both
quotas use hashed database keys; neither stores raw passwords or attempted
source IPs. A blocked client receives HTTP 429 and a Retry-After header.

**Important when using a reverse proxy:** the server sees the proxy's TCP
address, not each internet client's address. All clients behind that proxy
share Pulsewire's peer quota. Configure a separately verified per-real-client
rate limit at the trusted proxy; do not expose the app directly to public
traffic without trusted ingress controls. Pulsewire deliberately does **not**
accept untrusted `X-Forwarded-For` claims as source identity. A shared NAT
can also aggregate clients into one quota; account lockout is possible.

## Secret handling and debugging

An opt-in [file-backed PostgreSQL password](SECRETS.md) removes secret
**bytes** from the application's container environment, but does not hide
them from the Docker administrator, host filesystem access or a compromised
container. Default direct-environment credentials remain supported for
compatibility and are exposed through Docker metadata.

PostgreSQL configuration objects in the server and core/fetcher now redact
passwords in Rust `Debug` output. API auth payloads and token responses also
redact their passwords/bearer tokens in `Debug`. This limits accidental logging
but does **not** make raw credentials disappear from process memory, container
environment, Docker metadata, explicit HTTP token responses or unreviewed
startup/parse error paths. TOML parser and schema validation failures now
suppress source text and rejected values in both Display and Debug; this
protects secret-bearing config from being echoed in startup failures, at the
cost of less detailed error messages. Inspect sensitive files privately.
Do not print the full container environment or
provide real secrets in bug reports. Configure secrets privately at deployment.

An [opt-in loopback-only TLS ingress reference](INGRESS.md) now
documents an NGINX proxy, internal application bridge and CI-only
spoofed-forwarding-header negative tests. It does **not** authorize
public exposure, prove real-host firewall policy, or remove access by
containers already trusted on the database network.

The remaining `PW-023` ingress, TLS, metrics, and shared-network exposure
criteria are tracked in [P0 issue #9](https://github.com/sguzman/pulsewire/issues/9).
Do not interpret successful isolated CI smoke as approval to expose Pulsewire
publicly or migrate an existing household database.

## Requirements before starting on any host

1. Decide which **existing Docker network** can resolve the intended PostgreSQL
   service, and set `PULSEWIRE_DOCKER_NETWORK` to its exact network name.
   Pulsewire does not create or modify that shared network.
2. Set `PULSEWIRE_POSTGRES_HOST` to the database service's **Docker DNS hostname**
   on that network (default `postgres`). No router, CoreDNS or fixed resolver
   address is assumed.
3. Choose a **private PostgreSQL credential source**: an operator-provided
   mounted secret with `PULSEWIRE_POSTGRES_PASSWORD_FILE`, a nonempty
   `PULSEWIRE_POSTGRES_PASSWORD` environment value, or private TOML.
   A file and nonempty environment password **cannot** be combined. If using
   Compose secret-file overrides, see [SECRETS.md](SECRETS.md), ensure the
   source file is readable by the non-root container, and validate the
   overlay without starting it. An empty effective password fails at
   startup. Database usernames, database name and schemas still come from
   each service's `res/docker.toml` unless private config is provided.
4. Check that the selected PostgreSQL schema names are correct and that you
   have a current verified backup. The server currently requires the fetcher
   schema and tables, so initialize the fetcher **before** the server.
5. For any optional cookie/header files, supply the corresponding host
   directories only if used. Existing Compose examples retain those mounts.

Example preflight (with variables already supplied through your private shell
environment, **not** pasted real secrets):

```sh
docker compose -f crates/fetcher/docker-compose.yml config --quiet
docker compose -f crates/server/docker-compose.yml config --quiet
```

Review the resolved configuration before enabling either service. Normal
deployment uses `docker compose ... up --build -d` only **after operator
approval**. On an empty isolated database, run the fetcher to initialize its
tables, then bootstrap the first administrator with the server image's
`--bootstrap-admin` command while supplying
`PULSEWIRE_BOOTSTRAP_USERNAME` and `PULSEWIRE_BOOTSTRAP_PASSWORD` privately.
The bootstrap command exits without exposing an HTTP listener.

**Still not accepted for real deployment:** backup/restore on the actual
host dataset (isolated SQLite/PostgreSQL tests are green), repeatable upgrades
and rollback with real data, steady-state host resource measurements,
TLS/reverse-proxy and firewall trust boundaries, real-device/browser/TUI
human QA, and migration from an existing Pulsewire database. See
[RECOVERY.md](RECOVERY.md) for tested copy-only recovery, and
[COMPATIBILITY.md](COMPATIBILITY.md) for the verified platform matrix.

## Operator decision and read-only inspection

**Do not run the following preflight against an assumed home host or network.**
These are observations to use **after** operator approval identifies the exact
service, database, Docker network, backup destination and maintenance window:

- Record the exact source commit, local architecture, selected backend,
  deployment schema names, config locations, and installed binaries/images.
  The default Dockerfiles target `x86_64-unknown-linux-musl`; the current
  container CI uses a disposable PostgreSQL 16 database, not the owner's
  existing service.
- Review `crates/server/res/docker.toml` and
  `crates/fetcher/res/docker.toml` **without printing secret fields**.
  Both examples use the DNS name `postgres` and database `data`, with
  separate server/fetcher schemas. They are not guaranteed to match an
  actual home deployment.
- Review Compose with `config --quiet`, and separately verify that the
  selected external Docker network actually exists and the database service
  is reachable from the isolated test clients. Compose does **not** create
  that network or configure router/CoreDNS, TLS or firewall.
- Remember the internal/external bind distinction: the server listens
  on `0.0.0.0:8091` **inside** its container; Compose only publishes
  `127.0.0.1:8091:8091` at the host. Containers attached to the selected
  network can contact the internal port. HTTP traffic over an untrusted
  network is not approved. Fetcher metrics remain disabled/unpublished.
- Check private storage permissions and the existence of the optional
  cookies/headers directories **before** attempting an approved Compose
  startup. These are read-only mounts, but their contents are sensitive.

## First install versus upgrade (approval required)

On a completely **new, empty, isolated database**, fetcher schema initialization
comes first. The server then applies its own reader schema and a trusted
operator can invoke one-shot `--bootstrap-admin` with private credentials.
Do not use a default credential or ask the service to create an admin
automatically. The normal browser is accessible at `/reader` only once
the separately approved server is up, and API bearer authentication still
applies. A live `/health` success is process liveness, while `/ready`
also tests database connectivity but not the fetcher scheduler.

For an **existing** installation, do not treat `up --build` as a migration
plan. Before any write, record the current executable/image and config/schema
versions, take a consistent backup, rehearse a restore into a **new**
destination, run upgrades/migrations against that **copy**, and verify
accounts, feed items, subscriptions, favorites, folders and read state.
Do not assume old binaries can open newer migrated schemas. A rollback plan
restores a separately tested snapshot into a separately authorized location;
it does not DROP or overwrite a live shared database. See
[BACKUP-RESTORE.md](BACKUP-RESTORE.md).

## Privacy, troubleshooting and release boundary

Use [COMPATIBILITY.md](COMPATIBILITY.md) for exact config precedence,
supported CI environments, browser token storage and catalog defaults;
[TROUBLESHOOTING.md](TROUBLESHOOTING.md) for non-mutating diagnosis;
[MONITORING.md](MONITORING.md) for liveness versus readiness; and
[FETCH_SECURITY.md](FETCH_SECURITY.md) for outbound URL/DNS restrictions.
Do not leak Docker environment/passwords, RSS content, bearer tokens, cookies,
read state or SQL dumps. Public self-service password reset stays disabled:
[RECOVERY.md](RECOVERY.md) documents trusted offline recovery.

The operator must separately accept PW-023 (ingress/TLS/secrets/network),
PW-032 (actual target-host resource envelope), and PW-071 (deployment and
release) before any host change or tag. CI green is **not** approval to
start containers, publish ports, modify household networks or expose the API.
