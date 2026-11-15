# Pulsewire — backup, restore and retention operator runbook

**Status:** PW-043 procedures accepted in isolated CI fixtures only; actual host recovery, off-site durability and scheduled backups remain unverified.
Do **not** perform these operations on the household server without separate
operator approval, a maintenance window, and a verified off-host backup.

## Common safety preconditions

1. Identify the real database backend and data path/database name from your
   private configuration without printing passwords or tokens.
2. Confirm sufficient free space, destination ownership and write permissions.
   Write backups to a new path, not over the current database.
3. Verify the archive before stopping services or replacing any database.
4. Practice restore into an **independent** test database first, including
   subscriptions, favorites, folders, articles, read states and admin rights.
5. Never run DROP, TRUNCATE, `rm`, `wipe_database`, or an automatic reset on
   the original database as part of a recovery drill.

## SQLite consistent backup

The server's SQLite backup regression uses a **consistent online snapshot**
via SQLite's `VACUUM INTO` (rather than `cp` of a live WAL-mode DB):

```sh
# SQLITE_DB points to the existing file; SNAPSHOT is a fresh destination.
sqlite3 "$SQLITE_DB" "VACUUM INTO '$SNAPSHOT';"
sqlite3 "$SNAPSHOT" "PRAGMA integrity_check;"
```

Use a new, unique snapshot filename without SQL quote characters (the SQLite
CLI above interpolates a quoted filename into SQL); for paths with arbitrary
characters, use a parameterized SQLite client rather than shell interpolation.
A successful integrity check returns `ok`. Preserve file permissions, use
offline/encrypted/off-host storage as appropriate, and record the backup date.

For a **restore rehearsal**, stop any process using the *test destination*,
copy the snapshot to a new, unused SQLite filename, and point a disposable
Pulsewire instance at it. Run migrations and confirm item, read-state,
subscription, favorite, folder and administrator invariants before considering
a real restore. Never overwrite the active database in place while WAL is
open. Migrations may change data and must be rehearsed on restored copies.

## PostgreSQL custom-format backup

Prefer a new archive and a separate restore database. The disposable
container smoke already exercises the equivalent `pg_dump`/`pg_restore`
flow and checks feed items, read states, subscriptions and favorites.

```sh
# Set connection details via a private PGSERVICE / .pgpass, not a DSN
# containing a password in command history or logs.
pg_dump --format=custom --file "$BACKUP_FILE" --dbname "$SOURCE_DB"
pg_restore --list "$BACKUP_FILE" > "$BACKUP_FILE.contents"
# RESTORE_DB must be a NEW, EMPTY, separately authorized scratch database.
createdb "$RESTORE_DB"
pg_restore --exit-on-error --no-owner --no-acl \
  --dbname "$RESTORE_DB" "$BACKUP_FILE"
```

The supplied identities must have privileges to read every source schema and
restore into the scratch database. If the source contains external extensions
or role grants, document those separately. Do not create/restore into an
existing database containing data and do not publish dump files as CI
artifacts: archives contain authentication hashes and private feed metadata.

After restoration, compare exact per-table counts between source and scratch
DB for `fetcher.feeds`, `fetcher.feed_items`, `fetcher.feed_payloads`,
`server.users`, `server.subscriptions`, `server.entry_states`,
`server.favorites`, `server.folders` and `server.folder_feeds`.
Verify a real reader through a disposable authenticated API and rehearse
migration replay against the **copy**, not the original.

## Retention policy (conservative default)

The current repository interfaces insert feed payloads, items, fetch events
and optional state history. **Do not assume automatic age-based pruning or
bounded database growth**: there is no accepted, audited retention job in
the current PW-043 scope.

Until a specific deletion policy and FK/read-state preservation tests pass:

- **Preserve** feeds, items referenced by read states, subscriptions, favorites
  and folder membership; never prune authentication/recovery state ad hoc.
- Treat historical payloads, fetch events and feed-state history as
  *potential* retention candidates, not approved deletions. Back up and
  test constraints before ever introducing a prune operation.
- Monitor size and growth rather than automatically deleting data.
- Snapshot/restore drills must show that historical data and reader state
  survive, not merely that the database starts.

## Migration compatibility and rollback boundary

SQLite and PostgreSQL schema migrations are covered by small, isolated
CI fixtures, and the restart and API/TUI tests verify preservation of
those synthetic records. They are **not** evidence that arbitrary older
Pulsewire databases can be upgraded or downgraded in place, or that a
large real dataset will fit a maintenance window. Treat a service release,
config/schema pair, and database snapshot as a single versioned unit.

For a future approved upgrade, retain a source-compatible copy of the
original configuration and record the exact deployed commit and database
snapshot identifier before any migration. Rehearse against an isolated
new destination; verify SQL integrity and HTTP reader state after **both**
fetcher/server migrations. Never try an older binary on an already migrated
real database without a separately validated downgrade; favor an
independently restored **copy** and a rollback rehearsal. No automatic
pruning or destructive recovery is part of the accepted contract.

## What CI proves and what it does not

The existing server SQLite test performs an isolated `VACUUM INTO` snapshot,
restores to a different file and replays migrations, verifying read-state and
admin rights; PW-043 extends this to subscriptions/favorites/folders.
The disposable PostgreSQL container smoke uses `pg_dump -Fc`,
`pg_restore --exit-on-error` into `data_restore` and compares operational
data presence; PW-043 extends that to exact per-table counts.

These tests prove snapshot mechanics on small synthetic databases. They do
**not** prove backup encryption, off-site storage durability, large real-world
database restore times, live maintenance safety or a production retention
policy. Those require separate operator-approved validation.
