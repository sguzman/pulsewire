# PW-043 — Migration, retention and backup recovery acceptance

**Verified complete in isolated test environments, 2026-11-14.**
Roadmap milestone M4 / PW-043 carries **2 weighted points**. Its completion
does **not** approve production deployment, a backup job on an actual host,
or age-based data deletion.

## Verified end-to-end recovery evidence

| Property | SQLite fixture | PostgreSQL fixture |
|---|---|---|
| Nonempty source data | Reader, feed, article, current read-state, subscription, favorite, folder and folder-feed relation; payload, fetch event and link-state history | Fetcher RSS ingestion and authenticated reader HTTP interactions |
| Consistent backup | `VACUUM INTO` online snapshot to a new file | `pg_dump --format=custom` |
| Non-destructive restore | Copy snapshot to a separate SQLite file | `pg_restore --exit-on-error` into separate disposable `data_restore` |
| Schema migration/replay | Idempotent server schema replay; legacy `is_admin` migration does not confer admin until credential proof | Fetcher/server PostgreSQL migration and service integration, plus schema restore |
| Data-preservation checks | Article, read-state, admin rights, subscriptions, favorites, folder membership, historical payload, fetch event and state history | Exact original-vs-restored row counts for feed, payload, item, current state, historical state/event, reader, subscription, read-state, favorite and folder tables; nonempty reader/feed assertions |
| Isolation | Temporary test-only SQLite files, deleted after test | Disposable PostgreSQL container/network, separate restore database, destroyed after CI |

## Source and CI provenance

- The SQLite migration-and-recovery test already existed in
  `crates/server/src/schema.rs` and was **extended** by
  [PR #30](https://github.com/sguzman/pulsewire/pull/30). The
  `VACUUM INTO` snapshot and separate-file restore **predate** this PR;
  the new tests add reader relationships and historical preservation.
- The PostgreSQL `pg_dump` / `pg_restore` step already existed in
  `.github/workflows/containers.yml`. [PR #30](https://github.com/sguzman/pulsewire/pull/30)
  adds exact original/restored table-count comparisons, including
  historical fetch and state tables.
- PR #30 merged as
  [c8b2724](https://github.com/sguzman/pulsewire/commit/c8b2724f205f4cd8b4ccfa0961aab7cbbfaf42d9).
- [Rust checks 43617607376](https://github.com/sguzman/pulsewire/actions/runs/43617607376)
  **completed successfully**, including Rust format, compile, unit/integration
  tests, synthetic backlog, PostgreSQL acceptance and Clippy.
- [Container packaging and smoke 43617607458](https://github.com/sguzman/pulsewire/actions/runs/43617607458)
  **completed successfully**, including full disposable PostgreSQL
  authenticated HTTP integration and independent backup/restore assertions.

## Retention policy and operational boundaries

[BACKUP-RESTORE.md](https://github.com/sguzman/pulsewire/blob/main/docs/pulsewire/BACKUP-RESTORE.md)
documents operator preconditions and copy-only recovery drills for both
backends. The accepted **retention policy is preservation by default**:
the application currently does not have an approved automated pruning job.
The fixture explicitly checks that historical payload/event/state rows
survive migrations, backups and restores. No DELETE/TRUNCATE-based
age retention policy is approved or enabled.

The following remain outside this milestone: verifying encryption/off-site
backup durability, profiling realistic database size or restore speed,
running drills on a real host, configuring automated schedules, approving
deletion policies, and verifying ingress/TLS network exposure. Those require
separate operator approval or milestone acceptance.

**No household database, container, network, CoreDNS, feeds or production
file was touched in this acceptance.**
