# Pulsewire Core (pulsewire-Core)

Shared runtime library for the fetcher and server. This crate owns the domain model, scheduler logic, and infrastructure adapters (DB, HTTP, clock, RNG).

## What It Does

- Parses and validates the TOML config bundle (app/domains/categories/feeds).
- Owns the scheduler that decides when to HEAD/GET feeds and applies backoff.
- Defines domain types (feeds, state, HTTP results, errors) and ports/traits.
- Provides SQLite/Postgres repos and SQL schema application.

## Key Modules

- `app/` – scheduler orchestration and context wiring.
- `domain/` – core types (feeds, poll/backoff rules, state machine decisions).
- `ports/` – trait definitions for repos, HTTP, time, RNG.
- `infra/` – concrete implementations (sqlx repos, reqwest HTTP, logging, config
  loader).
- `res/sql/` – database DDL for SQLite and Postgres.

## Extension Points

- Implement `ports::repo::Repo` to support a new backend.
- Implement `ports::http::Http` to change the HTTP stack.
- Implement `ports::clock::Clock` for custom time or test control.

## Tests

- Property-style tests live under `crates/core/tests`.

## Usage
This crate is not a standalone binary. Use it through `crates/fetcher` (fetcher daemon) or `crates/server` (HTTP API server).
