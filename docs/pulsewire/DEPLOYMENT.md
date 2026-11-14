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

The fetcher now applies conservative source URL and redirect checks plus
bounded streaming. Read [FETCH_SECURITY.md](FETCH_SECURITY.md) before allowing
third-party source definitions or granting the container unrestricted outbound
network access. A syntactically public hostname can still resolve to private
IP addresses, so this application-layer policy is not a substitute for
network egress filtering.

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
