# PW-023 — read-only Docker network/ingress preflight

**State:** source-only read-only inspection tooling; **not deployment
approval**. The project's accepted score remains **93/100 (Δ +0)**,
and PW-023 remains an open **0/3** weighted security gate.

## What the scripts do — and do not do

- [pw023-capture.sh](../../scripts/pw023-capture.sh) reads **only**
  selected container network attachments, **effective runtime**
  host port bindings from `NetworkSettings.Ports`, running state, and
  selected Docker network membership/internal flags. This includes
  automatic `-P` / `--publish-all` port assignments that may be absent
  from `HostConfig.PortBindings`. Exposed-but-unpublished ports have
  null runtime mappings and do not count as host publications.
  The **top-level** `NetworkSettings.Ports` and
  `NetworkSettings.Networks` fields must be inspected JSON objects;
  a missing/null/array port map, malformed mapping value or
  publication without string `HostIp` and `HostPort` is rejected,
  not silently rewritten into an empty publication list. Null
  entries for individual unpublished `EXPOSE` ports remain valid.
  It uses Docker inspect formatting; it does **not** inspect the
  application environment, mounted secret paths or values, image
  labels, container logs, files, databases or firewall rules.
  **No Docker, firewall, network, process or file changes** are made.
  It prints a small JSON inventory to stdout, which can still reveal
  **private container/network names**. Treat a saved snapshot as
  sensitive operational metadata.
- [pw023-audit.sh](../../scripts/pw023-audit.sh) reads only that
  sanitized JSON snapshot, using jq. It makes **no Docker calls**.
  Supply an **explicit, human-reviewed** list of containers allowed
  on the database network; this cannot be guessed or inferred from
  whatever containers happened to be attached at scan time.
  No password, token or private config is required for the audit.
- The audit checks that the running reader has **no published HTTP
  port**, the TLS proxy has exactly one **127.0.0.1-bound 8443/tcp**
  binding with a decimal host TCP port in 1–65535 (not a missing,
  malformed or otherwise nonempty value), app↔proxy separation is
  as designed, the app bridge is internal with exactly the expected
  two members, and the proxy cannot directly join the DB network. When monitoring is
  included, it checks that fetcher/Prometheus share an **internal**
  monitoring bridge, have **no host-published ports**, and that
  Prometheus is not on the database network. Both monitoring peers
  must be running, role names must be nonempty and distinct, and
  returned Docker network names must match the requested inventory
  identifiers. Monitoring port metadata must be an **explicit empty
  array** for each peer; missing, null or malformed values cannot
  establish that a port is unpublished. Inconsistent or partial
  metadata fails closed.
- The scanner **never performs traffic probes or port scans**.
  It does not read nftables/iptables, IPv6 host rules, router/NAT
  policy, Docker daemon privileges, actual TLS certificate
  issuance/renewal, real upstream client-IP trust, bearer-token
  rotation or filesystem/TSDB permissions. It does not certify that
  containers on the **trusted database network** cannot bypass
  the TLS proxy. These remain separate, owner-approved gates.

These scripts are **optional evidence collection tools**, not an
instruction or authorization to access or modify the owner's home
infrastructure. In particular, no running host was inspected by
merely merging these scripts into the repository.

## Evidence from existing snapshots (Docker not needed)

CI includes
[synthetic sanitized inventory](../../tests/fixtures/pw023-network-audit.json)
with explicit, fictional container names and
[offline negative tests](../../tests/pw023-audit-smoke.sh) for
unauthorized DB membership, public 0.0.0.0 bindings, direct
server HTTP publication, an unexpected private-app member,
a stopped server or monitoring peer, mismatched network
identifiers, overlapping role names, and incorrect Prometheus
network/ports.



A separate [effective-port collector regression](../../tests/pw023-effective-ports-smoke.sh)
uses a fake Docker inspection command (not the Docker daemon) to test
that an `EXPOSE`-only port is ignored and that an automatically published
ephemeral `-P` host mapping is detected and classified `FAIL` even when
`HostConfig.PortBindings` is empty. It never publishes a real port.
The preflight still does **not** prove real reachability, firewall
behavior, IPv6, NAT or host policy.

The following **only reads that checked-in synthetic fixture**:

```sh
bash scripts/pw023-audit.sh \
  --snapshot tests/fixtures/pw023-network-audit.json \
  --allow-db ci-server --allow-db ci-fetcher \
  --allow-db ci-db-server
```

The output is structured JSON with a classification and individual
static checks. Return codes: **0** for `PASS_STATIC` with a supplied
and matching trust list, **1** for `FAIL` if a static boundary fails,
**2** for `NEEDS_REVIEW` when the DB-network trust list is missing
or has unreviewed members, and **64** for invalid invocation or an
unreadable/malformed snapshot. With no allowlist, the same fixture
must produce `NEEDS_REVIEW`, **not** a security approval.

A passing static report always includes
`"deployment_approved": false` and an `unknowns` array. It
does **not** certify TLS, host firewall, network routing or
operational safety.

## Optional future live read-only capture (separate owner decision)

A real-host snapshot requires the **exact** running reader/proxy
container names and application/edge/database Docker network
names. Supply these from an independently reviewed target environment,
not by guessing from this document. For monitoring, additionally
supply `--fetcher`, `--prometheus` and `--monitor-network`
as a group. **Do not run a host inspection without the owner's
authorization**.

The capture tool accepts:

```text
bash scripts/pw023-capture.sh \
  --server READER_CONTAINER --edge TLS_PROXY_CONTAINER \
  --db-network DATABASE_NETWORK --app-network APPLICATION_NETWORK \
  --front-network FRONT_NETWORK
```

It prints a sanitized snapshot; it does **not** automatically run the
audit or mark any member trusted. A reviewed snapshot may be passed
to `bash scripts/pw023-audit.sh --snapshot -` on stdin; the exact
`--allow-db NAME` flags must be chosen by a human familiar with
every selected database-network member. For a trusted member not
already on the network, do not create or attach it to "make the
audit pass." Investigate and decide intentionally.

**Never share the full output of ordinary `docker inspect`** when
checking exposure: that can include application environment secrets.
The capture script deliberately selects only specific safe metadata,
but network names/IDs can still be private. Treat all output as
owner-controlled, and review before uploading or publishing.

## Review/acceptance record

A CI-only live test invokes the collector **inside disposable Docker
fixtures** created for the existing TLS ingress test. It asserts the
actual containers match safe port and network rules, but with **no
operator-supplied database allowlist** the result must be
`NEEDS_REVIEW` and exit code 2. The offline tests independently
prove `PASS_STATIC` with an explicit allowlist, plus fail/review
cases for each boundary.

**Verified source and disposable CI (2026-11-15):**
[PR #60](https://github.com/sguzman/pulsewire/pull/60) squash-merged as
[`45d7d67`](https://github.com/sguzman/pulsewire/commit/45d7d6793ec04ab9750887c6c13c5dae80a3bacc).
The exact tested head `0afc0a20d316542201a0c93c6c2239d3c94848fe`
passed [Rust/docs/reader run 43777061823](https://github.com/sguzman/pulsewire/actions/runs/43777061823)
and [container run 43777061828](https://github.com/sguzman/pulsewire/actions/runs/43777061828).
The latter reports both synthetic trust-case acceptance and a **real
metadata-only Docker capture** inside the disposable ingress fixture,
correctly classified `NEEDS_REVIEW` without an operator DB-member
allowlist. See
[PW023-HOST-PREFLIGHT-ACCEPTANCE.md](PW023-HOST-PREFLIGHT-ACCEPTANCE.md).
No real household host was inspected. This grants **no weighted PW-023
acceptance credit** and no deployment permission.

For any future real-host evidence, preserve the exact code commit,
snapshot collection date and selected container/network identifiers;
explicitly review every database-network member, firewall routing,
TLS trust and metrics isolation. **No PW-023 weighted credit** is
awarded merely by capturing or classifying a snapshot. The relevant
[security issue](https://github.com/sguzman/pulsewire/issues/9)
must remain open until all P0 acceptance criteria and owner
authorization are satisfied.

See [INGRESS.md](INGRESS.md), [METRICS-SECURITY.md](METRICS-SECURITY.md),
[SECRETS.md](SECRETS.md), [DEPLOYMENT.md](DEPLOYMENT.md), and
[ROADMAP.md](ROADMAP.md).
