# PW-023 — TLS ingress and network isolation reference

**Status:** opt-in, loopback-only deployment reference with disposable
CI tests. **Not authorization to deploy, open a port or alter any home
service, router, CoreDNS, firewall or shared Docker network.** PW-023
remains **0/3 weighted points** until the actual-host trust boundaries
are independently accepted.

## What is being isolated

The optional [server Compose overlay](../../crates/server/docker-compose.ingress.yml)
removes the default host-published HTTP port from the reader and adds
an NGINX TLS proxy. It listens on port 8443 in the container and
publishes HTTPS to **127.0.0.1:8443 only** by default, not to the LAN
or the public internet. Its certificate and key are externally supplied
files; no certificates or credentials are checked into the repository.

| Container | Front bridge | Internal app bridge | Existing database network |
|---|---|---|---|
| NGINX TLS proxy | Attached | Attached | No |
| Pulsewire reader | No | Attached | Attached |
| Trusted PostgreSQL/fetcher | No | No | Attached |
| Untrusted edge client | Attached | No | No |

The internal bridge is marked `internal: true`. The proxy is *not*
a member of the PostgreSQL network. The reader retains PostgreSQL
connectivity, but loses its direct host HTTP publication. Only the
proxy publishes a port, and that port is explicitly bound to host
loopback. Container CI exercises this topology independently of the
household networks.

**Important limitation:** a container that is already on the
database network can still reach the Rust reader's internal HTTP
listener because it binds to `0.0.0.0:8091` in its container.
Anyone with Docker administrator permissions can attach a container
to that network or inspect mounted secrets. This profile does **not**
provide security against an untrusted database-network member or a
host/Docker administrator. A real deployment requires an inventory of
every attached container, strict network permissions and separately
verified firewall controls.

## TLS and exposed paths

[NGINX configuration](../../deploy/pw023/nginx.conf) accepts TLS 1.2+
and uses files mounted as `/etc/nginx/tls/cert.pem` and
`/etc/nginx/tls/key.pem`. It provides no ACME issuance, renewal,
public DNS name, NAT mapping or external reverse proxy. CI generates
a short-lived, synthetic self-signed certificate and verifies the
server certificate when making HTTPS requests. Do not reuse the CI
certificate for a real deployment.

Only `/reader`, `/reader/*` and `/v1/*` are proxied to the real
Rust server. `/health`, `/ready`, `/docs`, `/openapi.json`,
`/metrics` and unknown paths receive 404 at the edge. Readiness
remains available only to approved internal operators. The proxy
sets HSTS, nosniff and no-referrer response headers. Access logging
is disabled so sensitive URLs or user tokens are not incidentally
recorded by NGINX. HTTP request body size and upstream timeouts
are bounded; this configuration is a starting point, not a complete
internet-facing security review or firewall.

## Authentication rate limiting

The proxy limits `/v1/auth/login` to **one request per second per
actual TCP peer**, with a burst of three before HTTP 429. It uses
NGINX `$binary_remote_addr`, **not** arbitrary
`X-Forwarded-For`, `X-Real-IP` or `Forwarded` headers.
All three client-supplied forwarding headers are cleared before
the request reaches Rust. The server independently retains its
own five-attempt/15-minute per-username and 60-attempt/15-minute
per-peer quotas. The Rust per-peer quota sees the proxy as **one
shared client**; the edge limit is the separately enforced
per-real-client safeguard, not a substitute for Rust account limits.
The database-backed username and TCP-source quotas use separate
hashed key namespaces (`login-user:` and `login-source:`).
An allowed account name such as `login-source:192.0.2.10` must
never alias the same IP's source counter: successful login clears
only that username's quota, not the source-IP quota. Regression
tests exercise this exact collision. Switching username keys
to a prefixed format resets any pre-existing per-username
counters once on upgrade; the independently keyed source
quota stays intact. This is an intentional 15-minute-window
compatibility trade-off, not actual-host security acceptance.

For nonempty login attempts, a missing username and an existing
username with a wrong password now both perform an Argon2 verification
before returning the same HTTP 401 invalid-credentials response.
The absent-user branch verifies a fixed dummy Argon2id hash with the
current default work parameters, but **never issues a token** even if
that dummy's preimage is submitted. Unit tests pin the dummy and
current password-hash cost parameters; disposable PostgreSQL/SQLite
API tests assert identical public responses for both failure cases.
This mitigates an obvious timing-enumeration signal; it is **not**
a claim of constant-time end-to-end login behavior (database lookups,
hash parameter variations and network delays can still differ).
All failed login attempts remain subject to existing throttling;
the heavier unknown-user path makes source/edge limits important.

The CI smoke makes requests from two distinct Docker client IPs:
spoofing a new forwarding IP on every login attempt must **not**
avoid the first client's edge HTTP 429; the independent second
client must still receive normal HTTP 401 for invalid credentials.
An upstream CDN or second proxy would change the real connected
peer. Configure a tightly scoped trusted-peer forwarding policy
only after validating that external architecture; never blanket
trust arbitrary forwarded headers.

## Safe preflight

In a future separately approved environment, provide the *actual*
existing `PULSEWIRE_DOCKER_NETWORK`, a private database credential
source and private paths for `PULSEWIRE_TLS_CERT_SOURCE` and
`PULSEWIRE_TLS_KEY_SOURCE`. The optional
[secret-file overlay](../../crates/server/docker-compose.secrets.yml)
can be composed alongside this profile for PostgreSQL credentials.
With variables already supplied privately, the following command
**only checks configuration** and starts nothing:

```sh
docker compose -f crates/server/docker-compose.yml \
  -f crates/server/docker-compose.ingress.yml config --quiet
```

Review the resulting configuration *without printing secrets*: the
reader must publish **zero** ports, the edge must publish **only**
the 127.0.0.1-bound HTTPS port, and the database network must be
external and explicitly chosen. The use of `!reset []` requires
a Docker Compose version that supports the reset tag; failure to
clear published HTTP is a hard preflight failure. A passing config
check does not validate certificate trust, Docker firewall behavior
or live-host data safety and is **not** a deploy command.


## Exact-head isolated ingress verification (2026-11-15)

[PR #58](https://github.com/sguzman/pulsewire/pull/58), squash merged
as [`22e90ee`](https://github.com/sguzman/pulsewire/commit/22e90ee4eb0fb9c66f3d7247ab7cd613ad061c7d),
passed both [Rust/docs/browser checks 43767148246](https://github.com/sguzman/pulsewire/actions/runs/43767148246)
and [container smoke 43767148289](https://github.com/sguzman/pulsewire/actions/runs/43767148289)
on exact head `7001d89b4df79eefe3ba5ef1b1f03ef6a79602b7`.
The container log includes `PW-023 TLS ingress passed`. Test evidence
covers live Rust reader behind NGINX with test-only self-signed TLS,
actual client certificate validation, blocked private routes, loss of
direct host HTTP publication, distinct edge TCP-client IPs, per-IP
HTTP 429 despite forged forwarding headers, and inability to reach the
reader's internal app-network alias/IP directly.

**Scope is only disposable CI.** Containers already authorized on the
**database network** may still access the reader without passing through
the proxy. No home Docker network membership, host firewall, actual TLS
certificate/renewal, public hostname, metrics access-control policy or
production exposure has been verified. PW-023 remains **active, 0/3
weighted points**, and the project remains **93/100 (Δ +0)**.

## Metrics and remaining acceptance blockers

Fetcher metrics are disabled by default, and the TLS edge never
proxies `/metrics`. When separately enabled off-loopback, the
[authenticated metrics profile](METRICS-SECURITY.md) requires a
file-backed token at the Rust listener itself and configures a
private Prometheus scraper. That does not prove host firewall rules,
Prometheus UI access or token lifecycle. The separate
[monitoring guide](MONITORING.md) covers CI-only private Prometheus
scraping; approved real-host credentials, membership, retention and
network restrictions remain unverified. A metrics process started on
the wrong Docker network could still leak information regardless of
this edge configuration.

An [optional read-only inventory preflight](HOST-PREFLIGHT.md) now
checks actual Docker network attachments and published ports against
the source design, without reading secrets or changing the host.
It flags unreviewed DB-network members; it does not probe firewalls
or certify production TLS.

Before PW-023 closes, real-host evidence must independently verify
the firewall and every database/private network member, trusted TLS
certificate and key lifecycle, inbound proxy/client-IP trust,
per-client rate limits, metrics isolation and retention, and
owner-approved network configuration. PW-032 real-host resource
measurements and PW-071 owner-approved deployment remain separate.
No source change or disposable CI test is permission to publish
Pulsewire publicly.

See [DEPLOYMENT.md](DEPLOYMENT.md), [SECRETS.md](SECRETS.md),
[FETCH_SECURITY.md](FETCH_SECURITY.md), and [ROADMAP.md](ROADMAP.md).
