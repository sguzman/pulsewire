# Pulsewire Roadmap

## Phase 0: Foundation (current)
- Fetcher + server + TUI operational
- Config schemas + validation
- Metrics and logging stabilized

## Phase 1: SemVer + Release Tooling
- Conventional commit enforcement
- git-cliff changelog generation
- cargo-release workflow
- Documented release policy

## Phase 2: Client API Stabilization
- Freeze OpenAPI surface
- Error codes and pagination finalized
- Compatibility notes for clients

## Phase 3: Clients
- Desktop client
- Web client
- Shared API client library

## Phase 4: Calendar-based Fetcher
- Calendar provider adapter interface
- Release schedule ingestion + triggers
- Event-time retries and verification
- Calendar views in clients

## Phase 5: Realtime Sync
- WebSocket sync for subscriptions, entries, and unread counts
- Client-side state reconciliation

## Phase 6: Non-RSS/Non-Calendar Sources
- Ad-hoc connector interface
- Provider catalog discovery
- Per-source polling/trigger policies

## Future Ideas
- Additional data providers
- Connector marketplace
- Hosted service
