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

## Offline user password recovery

The unauthenticated HTTP password-reset endpoint remains disabled. A database
operator who already has access to Pulsewire's configured database may use the
server's `--recover-user` mode to set a new password for an **existing** user.
It requires `PULSEWIRE_RECOVERY_USERNAME`, `PULSEWIRE_RECOVERY_PASSWORD`
(minimum 12 bytes), and `PULSEWIRE_RECOVERY_CONFIRM=RECOVER_USER`. It makes
an atomic password update and deletes existing authentication and historical
reset tokens. It never creates an account, changes an administrator role, or
opens an HTTP listener.

Set the variables in a trusted private shell and invoke
`pulsewire-server --recover-user` using the normal private config. Do not put
a real password into a public terminal log, shell history, Git commit, or CI
configuration. This is **operator-managed recovery**, not a self-service reset.

### Verified operator-recovery drill (2026-11-13)

The recovery mode is verified against both SQLite (unit test) and isolated
PostgreSQL (regression and container runtime). [Rust CI 43525939594](https://github.com/sguzman/pulsewire/actions/runs/43525939594)
and [container CI 43525939629](https://github.com/sguzman/pulsewire/actions/runs/43525939629)
both passed. The container test proved that an invalid confirmation leaves the
database password hash unchanged, and that successful recovery revokes earlier
sessions and reset tokens while preserving the administrator role.

The command validates the confirmation and required credentials **before
opening the database**. A correct confirmation still presumes trusted operator
access to the database connection; it is not an authorization substitute and
must not be exposed through a web endpoint or untrusted job runner.

### Login-attempt throttling

Pulsewire keeps an atomic, per-username hashed attempt counter in the
configured SQLite or PostgreSQL database. Five attempts per 15-minute fixed
window are evaluated; later attempts return 429 with a Retry-After header.
Successful login clears the counter and old windows reset automatically.
The counter also covers unknown usernames, and failed credentials always
produce a generic 401 rather than identifying existing accounts.

Account-targeted denial of service is possible (a malicious client can
temporarily block a known username). Deploy a trusted network/reverse-proxy
rate limit for source-based protections; do not expose the API publicly without
one. Privileged offline recovery is unaffected by HTTP login throttling.

## Gates before production use

- Confirm the host's real storage topology, schema names, and backup permissions.
- Demonstrate a backup *and* restore on a fresh isolated copy of actual data.
- Validate database integrity, representative authenticated API reads,
  migration replay, application restart, and a rollback procedure.
- Define, measure, and test backup retention and scheduled backup verification.
- Record exact build SHA, data version, restoration outcome, and operator approval.

Until those checks pass, the isolated test success should **not** be treated
as production deployment readiness.
