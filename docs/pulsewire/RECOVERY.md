# Pulsewire — SQLite and PostgreSQL recovery

**Scope:** These are isolated, repeatable test patterns, **not** permission to
overwrite any running household database. Confirm your actual database name,
schemas, file paths, storage, backup credentials and restoration destination
before applying them outside a disposable environment.

## Current verification

On 2026-11-13, [Rust CI 43519275262](https://github.com/sguzman/pulsewire/actions/runs/43519275262)
successfully tested SQLite legacy-schema migration and a file-backed backup
via `VACUUM INTO`, restored into a **different file**. It verified the migrated
user ID, permission role, article and authenticated read state after migration
replay.

[Container CI 43519275224](https://github.com/sguzman/pulsewire/actions/runs/43519275224)
successfully exercised the real local RSS fetcher and reader API under
resource limits, and restored a PostgreSQL 16 `pg_dump -Fc` archive into a
**new database**, verifying articles, read states, subscriptions and favorites.

No actual household data was accessed. The tests did not prove real-host
restore procedures, a retention policy, upgrade rollback or cross-version
migration compatibility.

## PostgreSQL — isolated, non-destructive restoration drill

Use a PostgreSQL account with appropriate privileges and private credentials.
Do **not** put passwords in command-line arguments or commit them. These are
example database names, not the host's confirmed names:

```sh
pg_dump --dbname=pulsewire_test --format=custom \
  --file=/secure/backup/pulsewire_test.dump
createdb pulsewire_restore_check
pg_restore --dbname=pulsewire_restore_check --no-owner --no-acl \
  --exit-on-error /secure/backup/pulsewire_test.dump
```

Restore only into an **empty, newly created test database**. Check that the
expected fetcher and server schemas exist. Verify sample rows from `feed_items`,
`users`, `entry_states`, `subscriptions`, and `favorites`; then connect a
temporary isolated server to the restored database and verify authenticated
article access and read state. Do not drop or overwrite the source database.

## SQLite — consistent online snapshot and separate-file restoration

Use SQLite's online backup API or `VACUUM INTO`, rather than a filesystem copy
of a live SQLite file (which may be in WAL mode). The Rust CI test uses
`VACUUM INTO` with a bound destination parameter; the destination file
must **not already exist**.

For an isolated test file, the corresponding SQLite statement is:

```sql
VACUUM INTO '/secure/backup/pulsewire-test-snapshot.sqlite';
```

Use a correctly escaped, controlled destination path. Restore the snapshot
by copying it to a **different database file**, and verify
`PRAGMA integrity_check` reports `ok`. Replay migrations on the restored
copy, then verify article records, existing user IDs, roles and read state.
The new CI test repeats the server migration twice before and after recovery.

**Important:** Check both fetcher and server SQLite configuration. Their
checked-in example paths differ. The reader expects feed items and its user
state in an accessible database layout; independently backing up two files
does not provide a single atomic cross-file snapshot. Coordinate writers
and snapshot points for a multi-file installation.

## Gates before production use

- Confirm the host's real storage topology, schema names, and backup permissions.
- Demonstrate a backup *and* restore on a fresh isolated copy of actual data.
- Validate database integrity, representative authenticated API reads,
  migration replay, application restart, and a rollback procedure.
- Define, measure, and test backup retention and scheduled backup verification.
- Record exact build SHA, data version, restoration outcome, and operator approval.

Until those checks pass, the isolated test success should **not** be treated
as production deployment readiness.
