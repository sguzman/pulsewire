# PW-023 — file-backed PostgreSQL database credentials

**Status:** code-level, disposable-CI hardening only. **PW-023 remains active
(0/3 weighted points)** until separate TLS/reverse-proxy, trusted-container
network, metrics, firewall, edge-rate-limit and owner-approved host evidence
are complete. No household deployment is authorized by this procedure.

## Why use a secret file?

The original Compose examples inject PostgreSQL passwords directly into the
container environment. This is functional but container administrators can
read that value in Docker metadata such as `docker inspect`. Both Rust services
now accept `PULSEWIRE_POSTGRES_PASSWORD_FILE` as an opt-in **path** to a
mounted file, so the password **bytes** need not appear in the container
environment. The path remains inspectable.

This does **not** hide the password from the Docker administrator, root on the
host, another process with access to the mounted file, an attacker who has
compromised the container, or database memory/process state. Docker Compose
local-file secrets may be bind-mounted with source-file permissions rather
than being a true encrypted, access-controlled secret store. Use an
appropriate private host location and mode readable by container UID/GID
65532. Do not commit the file, print its contents or include it in logs.

## Runtime contract

The fetcher and server use the same implementation
(`crates/schemas/src/secrets.rs`):

1. If `PULSEWIRE_POSTGRES_PASSWORD_FILE` is set to a nonempty path, read the
   password from that file; a single trailing LF or CRLF line ending is
   removed. The secret must be a nonempty UTF-8 regular file, at most 4096
   bytes, containing no NUL or embedded CR/LF.
2. If **both** `PULSEWIRE_POSTGRES_PASSWORD_FILE` and nonempty
   `PULSEWIRE_POSTGRES_PASSWORD` are set, startup **fails**. This avoids
   ambiguous sources and an accidental environment secret.
3. With no file override, a nonempty `PULSEWIRE_POSTGRES_PASSWORD` wins.
   Otherwise, a private TOML `[postgres].password` is used for backwards
   compatibility. Empty PostgreSQL passwords are rejected during config
   validation.
4. Missing/unreadable/invalid files fail **closed**. An error is generic:
   it does not include the file path or password content. An explicit file
   failure never silently falls back to an environment or TOML password.

Password-file resolution is only for PostgreSQL. SQLite installations do
not need PostgreSQL credentials.

## Optional Compose overlays (preflight only)

The default Compose files continue to support the environment-password
workflow. To **opt in**, select the matching checked-in overlay:

- `crates/fetcher/docker-compose.secrets.yml`
- `crates/server/docker-compose.secrets.yml`

Each override clears the direct password environment variable, sets the
file path to `/run/secrets/pulsewire_postgres_password`, and mounts a
Compose secret from `PULSEWIRE_POSTGRES_PASSWORD_SOURCE`.

Choose a **pre-existing, private password file** for the intended PostgreSQL
user; ensure it can be read by UID 65532 within the container. The absolute
source path and password remain **operator inputs**, not values supplied by
this document. Check source file permissions on the actual platform.
Compose local secrets are not automatically a full managed secret lifecycle.

With `PULSEWIRE_POSTGRES_PASSWORD_SOURCE` and an **existing** approved
`PULSEWIRE_DOCKER_NETWORK` set privately, these commands **only validate**
the merged configuration and do not start containers:

```sh
docker compose -f crates/fetcher/docker-compose.yml \
  -f crates/fetcher/docker-compose.secrets.yml config --quiet
docker compose -f crates/server/docker-compose.yml \
  -f crates/server/docker-compose.secrets.yml config --quiet
```

The required `PULSEWIRE_DOCKER_NETWORK` points to a pre-existing network
shared with the selected database. These overlays **do not** create TLS,
reverse-proxy restrictions, container-network isolation, firewall policy,
private routing, metrics authorization or permitted public access. A server
still binds internally on all IPv4 interfaces, and any container sharing
its data network may reach it. Those are separate PW-023 blockers.

The default Compose files now allow an **empty** direct password environment
so the file overlay can be parsed without a dummy secret. If neither a valid
file nor a direct/private password exists, PostgreSQL-mode startup fails.
Never interpret a successful `docker compose config` as credential validity
or as authorization to run `up`.

## Verification and deployment boundary

The shared Rust helper unit tests check direct/TOML compatibility, a valid
file with CRLF, missing/unreadable/multiline file errors, and conflicting
environment/file inputs.

**Verified isolated acceptance, 2026-11-15:** [PR #57](https://github.com/sguzman/pulsewire/pull/57)
squash-merged at [`bae5abc`](https://github.com/sguzman/pulsewire/commit/bae5abc78137971b11119bcf5099b5bdd2b6dd43).
Its final source head `ef58659d8a8ad07fbce0247305b5d0a93c2cd367` passed both
[Rust/docs/Chromium CI 43765147667](https://github.com/sguzman/pulsewire/actions/runs/43765147667)
and [container CI 43765147773](https://github.com/sguzman/pulsewire/actions/runs/43765147773).
Container logs explicitly contain `PW-023 file-backed credentials: both real
binaries passed against disposable PostgreSQL`. The CI starts a fetcher with
a mounted secret and an independent reader server with a mounted secret,
checks `/health` and `/ready`, verifies the password bytes are absent from
the reader server's Docker environment metadata, rejects a missing explicit
secret file, and keeps the expected security errors generic.

**No PW-023 weighted acceptance is claimed:** 0/3 points; Pulsewire overall
remains **93/100 (Δ +0)**. No household database, secret, Docker network,
TLS configuration, proxy, firewall, public ingress, services or release were
changed. This proves only the file-secret mechanism in disposable CI.
 CI additionally exercises **both real service
binaries** against disposable PostgreSQL using a secret mount; it verifies
server SQL readiness and a completed fetcher benchmark while not injecting
the secret bytes into those containers' environment. The Compose overlay
is validated without starting any service. CI logs and source evidence
belong to PW-023, **not** PW-071 host deployment acceptance.

For other secrets: offline admin bootstrap/recovery still uses explicit
trusted operator environment variables and must be kept out of shell history,
logs and container inspection. This change does not introduce a
browser-managed credential vault or secure self-service account recovery.

See [DEPLOYMENT.md](DEPLOYMENT.md) for the full preflight,
[FETCH_SECURITY.md](FETCH_SECURITY.md) for outbound/private DNS restrictions,
[MONITORING.md](MONITORING.md) for protected metrics policy, and
[ROADMAP.md](ROADMAP.md) for the remaining PW-023 gate.
