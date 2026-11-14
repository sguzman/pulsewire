# Pulsewire — container deployment runbook

This is a **deployment preflight**, not permission to deploy. Do not run Pulsewire
against an existing household database until a backup and a restore test have
been completed (PW-043). Never run migrations or wipe tools against someone
else's PostgreSQL schemas.

## What is built and tested

- CI builds non-root musl fetcher/server images from the workspace using the
  locked dependencies. A disposable PostgreSQL service verifies initial fetcher
  database migration, offline admin bootstrap, server launch, login and denied
  unauthenticated account creation.
- The default fetcher image contains **non-YouTube, non-podcast** feed catalogs
  only. Original source definitions remain in Git. The loader also skips these
  categories when their directories are mounted.
- Ports are published only on **127.0.0.1** by Compose: server `8091`, metrics
  `9898`. A trusted reverse proxy on the Docker network may communicate with
  the server on its internal container port; there is no automatic public proxy.

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

PostgreSQL configuration objects in the server and core/fetcher now redact
passwords in Rust `Debug` output. API auth payloads and token responses also
redact their passwords/bearer tokens in `Debug`. This limits accidental logging
but does **not** make raw credentials disappear from process memory, container
environment, Docker metadata, explicit HTTP token responses or unreviewed
startup/parse error paths. Do not print the full container environment or
provide real secrets in bug reports. Configure secrets privately at deployment.

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
3. Supply `PULSEWIRE_POSTGRES_PASSWORD` securely in the container environment.
   The checked-in TOML examples contain an empty password. Database usernames,
   database name and schemas still come from each service's `res/docker.toml`.
   If your database uses different values, prepare a **private** config first.
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

**Not yet accepted for real deployment:** backup/restore against the actual
host database (isolated PostgreSQL and SQLite recovery tests are now green),
repeatable upgrades with real datasets, feed fetching at steady state,
TLS/reverse-proxy configuration, browser/TUI workflow QA, and migration from
any existing Pulsewire database. See [RECOVERY.md](RECOVERY.md) for the tested
recovery approach and its limitations.
