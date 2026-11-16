# Pulsewire — Nix development, packages, and local QA

**Scope:** reproducible Linux development and *build packaging*. Nix is not
a replacement for the already-tested Docker deployment, not a server rollout,
and not permission to access a household database or modify Docker networks.

The repository pins Nixpkgs 26.05 and rust-overlay in `flake.lock`. The
existing `rust-toolchain.toml` selects Rust nightly and its required
components; the lock pins the overlay's available toolchain snapshot. The
Docker image builders currently use Rust 1.91 and remain unchanged.
Keep the lockfile committed; update inputs deliberately with
`nix flake update` and verify the resulting build and CI.

## First use (Linux, flakes enabled)

From a fresh clone at the desired commit:

```sh
nix flake show
nix develop
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
nix build .#default
```

`nix develop .#qa` additionally provides PostgreSQL client/server tools
and Chromium for human QA. Neither shell starts PostgreSQL, Docker or
Pulsewire automatically. Node.js is included for the existing dependency-free
contract scripts; **no npm install is necessary**. The environment includes
pkg-config, CMake, Perl, OpenSSL, SQLite, jq, and just.

Nix package outputs on supported `x86_64-linux` and `aarch64-linux`:

| Output | Executable |
| --- | --- |
| `.#default` / `.#pulsewire` | All four executables |
| `.#fetcher` | `pulsewire` |
| `.#server` | `pulsewire-server` |
| `.#cli` | `pulsewire-cli` |
| `.#tui` | `pulsewire-tui` |

For example:

```sh
nix build .#server
nix run .#cli -- --help
```

`nix run .#fetcher` and `nix run .#server` **start real programs**.
Do not run these against a pre-existing database or enable live polling
without separately approved, isolated configurations. An empty PostgreSQL
password in the included examples is deliberately rejected at startup.

## Runtime resources and configuration

The single workspace build installs both binaries and their matching
versioned resource trees under `share/pulsewire/`:

- `config/fetcher/config.toml`, domain/category settings and a feed catalog
  **without the disabled YouTube/podcast definition directories**;
- `config/server/config.toml`;
- `config/tui/config.toml` and its validation schema;
- `schemas/fetcher/` and `schemas/server/` and `schemas/tui/`, including server SQL.

The fetcher and server use their existing executable-relative resource lookup
or a private operator-supplied `CONFIG_PATH`/`SERVER_CONFIG_PATH` and
`SCHEMAS_DIR`. The TUI package supplies its bundled non-authenticating
config by default; `PULSEWIRE_TUI_CONFIG` still overrides it. No passwords,
tokens, keys or private data belong in a Nix expression, `flake.lock`, or
the world-readable Nix store.

The packages are native Linux builds, not the static MUSL Docker images;
the original Dockerfiles, Compose overlays and CI remain authoritative for
container deployment. Nix does **not** create a database, register systemd
services, open ports, start a fetch loop, or touch CoreDNS/firewall state.
Docker is still the supported isolated PostgreSQL/network integration fixture.

## QA boundary and completion

The new Nix GitHub Actions workflow checks committed input locking, flake
evaluation and package construction. Existing Rust and container workflows
remain responsible for application integration and security regressions.

For the first local QA, review the checked-out commit and build the package.
Only after arranging a separate disposable database/network can we exercise
the live reader and feed ingester. Never point default Compose examples
at the existing household PostgreSQL network just to start a test.

This addition is **not** PW-023, PW-032 or PW-071 host acceptance:
the weighted project score remains **93/100 (Δ +0)** until independently
verified, owner-approved actual-host acceptance.
