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

## Important residual risk: DNS and network egress

The URL policy checks URL strings and IP **literals**. It does **not**
inspect each DNS resolution or pin the resolved IP on connection. A
syntactically public hostname may resolve to a private address, and
DNS rebinding can change that resolution. This includes Docker-only
hostnames and external hostnames controlled by an attacker.

Consequently, do **not** permit arbitrary unauthenticated users to
submit sources or run the fetcher on an unrestricted trusted LAN.
Require deployment-level outbound firewall/proxy rules blocking
loopback, RFC1918, link-local and metadata services for the fetcher
network. A trusted internal source would need an explicitly reviewed
configuration/policy extension; do not silently weaken the production
default to admit private URLs. Keep local services off the fetcher's
unrestricted network until the DNS/egress boundary is independently
verified.

The isolated CI feed fixture uses an internal Docker DNS name
(`ci-rss`). Passing this fixture **does not** establish private-IP
egress isolation. The default YouTube and podcast catalogs remain
disabled and preserved.

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

The production constructor never enables the test-only local-address
override. The existing Docker smoke validates actual local RSS
fetching and disabled-category suppression in a disposable network.

## Remaining release gates

* DNS/IP resolution filtering or independently verified egress controls.
* Feed-by-feed compatibility QA for redirect strictness.
* Browser/TUI and steady-state fetching tests and real-host backups.
* Separate owner approval before modifying running home services.

See [ROADMAP.md](ROADMAP.md) for the weighted implementation ledger.
