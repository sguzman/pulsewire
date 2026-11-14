# Pulsewire — outbound fetch security

This is the security contract for the **general news/RSS fetcher**. It is
an implementation and test description, not a claim that the service is
ready for production or safe to expose to untrusted feed-submitters.

## URL policy

Source URLs are parsed as URLs (not string-split hostnames). The fetcher
accepts only absolute HTTP/HTTPS URLs with a host, no username/password
component, no fragment and no surrounding whitespace. URLs over 8192
bytes are rejected. Source ingestion and actual network requests use
the same validation. Literal loopback, private, link-local, reserved,
unspecified and multicast addresses are refused, including IPv4-mapped
IPv6. Local names such as `localhost`, `*.localhost`, `*.local`,
`*.internal`, and `*.home.arpa` are also refused.

Automatic redirects are limited to five hops. A redirect must remain
on the same origin, except for a same-host HTTP/80 → HTTPS/443 upgrade.
HTTPS downgrades, other hosts, arbitrary port changes and unsafe target
URLs are refused. This conservative policy is intentional: automatically
forwarding custom authentication headers or cookies to a new origin
would be unsafe. If an upstream feed begins redirecting across hosts,
update the source definition to its intended canonical URL after review.

## Resource ceilings

The HTTP adapter preserves the prior **30-second total request timeout**
and adds a **10-second connection timeout**. Response bodies are read
incrementally rather than through an unbounded `bytes()` collection.
The default **maximum accepted decoded body is 8 MiB**; if a response
advertises a larger Content-Length, it is rejected before reading the
body. Chunked bodies with missing or misleading Content-Length are
discarded once the accepted-byte ceiling would be crossed. No partial
body is persisted or passed to the parser on overflow.

HTML-watch `max_body_bytes` can impose a lower ceiling per source;
it cannot increase the global ceiling. The scheduler validates body size
again before parsing, even for custom or mocked HTTP adapters. The RSS
parser also independently rejects inputs above the global ceiling.
Oversize/invalid requests currently use the general `Unexpected`
error classification and the existing scheduler backoff; no new
classification or HTTP API endpoint was added.

These bounds limit the byte buffers accepted for parsing. They do **not**
prove a strict process-wide memory limit: decompression, malformed XML,
HTML DOM expansion, or many concurrent requests can still consume more
memory. Keep the fetcher container resource limits in place.

## Connection-time DNS and network egress

The fetcher's strict Reqwest client now supplies a **custom DNS resolver**
(`crates/core/src/infra/guarded_dns.rs`) that validates the addresses
actually handed to Reqwest for a connection. It rejects an empty result
or any non-public IPv4/IPv6 answer, including private, loopback,
link-local, documentation/reserved and metadata-service addresses. This
also rejects a mixed public/private DNS answer instead of trusting only
its public entries. Each fresh DNS resolution is checked, including
resolutions requested after allowed redirects. The client disables
automatic ambient HTTP(S) proxy discovery, which otherwise could allow
a proxy to resolve forbidden targets outside these checks.

For **trusted internal feeds only**, an operator can explicitly set
`PULSEWIRE_ALLOW_PRIVATE_FEEDS=1`. This relaxes both source URL
validation and resolver address restrictions; absent or other values
keep the strict default. The isolated container smoke sets this opt-in
for the intentionally local Docker fixture `ci-rss` and does **not**
prove strict-mode access to that fixture. Never set this switch merely
to make an untrusted external feed work.

This guard is an application-level egress restriction, **not** a network
sandbox or a proof that every library/process can only contact public
destinations. Keep defense-in-depth firewall/network egress rules that
block private, link-local and metadata targets, especially before
untrusted users can introduce feeds. Review proxy, container and DNS
trust boundaries during PW-023 deployment hardening. The default
YouTube and podcast catalogs remain disabled and preserved.

## Automated evidence

`crates/core/src/infra/http_policy.rs` tests reject malformed,
credential-bearing and unsafe literal URL targets, plus cross-host,
downgrade and nonstandard-port redirects.

`crates/core/src/infra/reqwest_http.rs` uses an isolated
`127.0.0.1` HTTP test server via a test-only constructor. It checks:
- an exactly permitted chunked body is accepted;
- a chunked body one byte too large is discarded;
- an oversized declared Content-Length is rejected;
- a cross-host redirect fails;
- direct loopback targets fail under the production policy.

The production constructor enforces the strict policy by default and
accepts private feeds only with the explicit operator opt-in. DNS
regressions in `crates/core/src/infra/guarded_dns.rs` test acceptance
of a public answer, rejection of private/metadata and mixed public/
private answers, empty answers and strict localhost resolution. The
Docker smoke opts in for its trusted internal RSS fixture, validating
end-to-end ingestion and disabled-category suppression in an isolated
network. Both Rust and container CI passed on PR #4.

## Remaining release gates

* Independent network-level egress filtering and proxy/DNS hardening
  before running on a trusted host or accepting third-party sources.
* Feed-by-feed compatibility QA for redirect strictness and internal
  feeds that require deliberate private-network opt-in.
* Browser/TUI and steady-state fetching tests and real-host backups.
* Separate owner approval before modifying running home services.

See [ROADMAP.md](ROADMAP.md) for the weighted implementation ledger.
