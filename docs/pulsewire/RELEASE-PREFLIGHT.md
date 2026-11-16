# PW-071 — read-only release readiness and manual approval

**Current status: BLOCKED.** The canonical weighted roadmap is **93/100**,
not a release-ready product. PW-023 (P0; 3 points), PW-032 (P1; 2
points), and PW-071 itself (P1; 2 points) remain unaccepted. No home
deployment, target-host restore drill, owner-approved release or
published version/tag has been performed as part of this work.

## Safe source-only inspection

From a normal checkout, with Node available:

```sh
node scripts/pw071-release-preflight.mjs --status
```

Or use `just release-status` if Just is installed. These commands
only read the canonical [roadmap](ROADMAP.md), check that its 27 task
weights sum to 100 and its accepted weight agrees with the declared
score, then print a JSON status. Neither command creates or changes
files, versions, tags, releases, containers, databases, Docker networks,
firewall rules or remote state. **The command exits successfully for
a valid roadmap even when release is blocked.** Read the JSON
`outstanding` and `release_approved:false` fields: a zero exit code
is **not** release authorization.

The old root Just `major`, `minor` and `patch` recipes previously
executed `cargo release --execute`, `git push`, and
`git push --tags`. They are now deliberately changed to **fail-closed
readiness-only** commands:

- They check all P0/P1 milestones; unfinished tasks produce exit
  code 2 before any release action. PW-023, PW-032 and PW-071 are
  currently still open.
- If the ledger is someday fully accepted, they additionally require
  a clean `main` checkout, a private **external** approval receipt,
  and a receipt claiming the *exact current commit* and release type.
- Even if that future declaration is well formed, the recipes
  only print a message. They **never** bump the version, execute
  cargo-release, commit, tag, push, publish, deploy or restart.
  A real release must follow a separate, explicitly approved
  manual procedure in [RELEASE.md](../reference/RELEASE.md).

The preflight's checks are **safeguards, not attestations**. It cannot
independently authenticate a human owner, confirm GitHub CI for a
future commit, inspect household ports/containers, validate TLS
renewals, prove host restore, or certify resource budgets.

## What a future private review receipt must declare

After **all** P0/P1 milestone acceptance and independent target-host
review, an operator can prepare a private JSON file stored **outside
the Git repository** with owner-only file permissions. The schema
has the following shape (placeholders are **not approved evidence**):

```json
{
  "schema": 1,
  "repository": "sguzman/pulsewire",
  "commit_sha": "REPLACE_WITH_EXACT_40_HEX_COMMIT",
  "release_type": "patch",
  "owner_approved": false,
  "approval_recorded_at": "YYYY-MM-DDTHH:MM:SSZ",
  "accepted_gates": {
    "pw023_host_exposure": false,
    "pw032_target_resource_envelope": false,
    "pw071_host_deployment": false,
    "backup_restore_target_host": false,
    "exact_commit_ci": false,
    "owner_release_authorization": false
  },
  "evidence_references": {
    "pw023_host_exposure": "REPLACE_WITH_REVIEWED_EVIDENCE",
    "pw032_target_resource_envelope": "REPLACE_WITH_REVIEWED_EVIDENCE",
    "pw071_host_deployment": "REPLACE_WITH_REVIEWED_EVIDENCE",
    "backup_restore_target_host": "REPLACE_WITH_REVIEWED_EVIDENCE",
    "exact_commit_ci": "REPLACE_WITH_REVIEWED_EVIDENCE",
    "owner_release_authorization": "REPLACE_WITH_REVIEWED_EVIDENCE"
  }
}
```

The gate requires `owner_approved:true`, all six boolean gates
explicitly accepted, and reviewed HTTPS or SHA-256 evidence references
for each gate. It also requires the exact source SHA/type, a UTC
timestamp, a clean checked-out `main`, an absolute receipt path
outside the repository, and owner-only file permissions (0600).
The receipt **cannot** grant itself trusted status: an offline
parser verifies its shape and consistency, **not** its human
authenticity or claimed remote/host evidence. Never commit a real
approval receipt or real host metadata.

To validate such a **future** private declaration without creating
a release, the approved operator could set
`PULSEWIRE_RELEASE_APPROVAL_FILE` to its absolute private path and
invoke `just patch` or the matching type. That still **does not
release anything**. No receipt exists or is needed for the current
blocked state.

## Actual outstanding work

1. **PW-023, 3 points:** review the real host's Docker trust
   membership, firewall/NAT/IPv6 reachability, TLS certificate
   lifecycle and proxy forwarding chain, metrics UI/transport and
   secret permission/rotation policy. The disposable
   [host preflight](HOST-PREFLIGHT.md) does not prove this.
2. **PW-032, 2 points:** approve and measure actual target-server
   CPU, RAM, isolated Pulsewire DB growth, backlog and poll-latency
   on a sustained representative news/government/journals workload,
   not the [brief CI sample](PW032-DISPOSABLE-ACCEPTANCE.md).
3. **PW-071, 2 points:** verify exact release-commit CI and accepted
   P0/P1 gates; prove target-host deployment and its **real** backup
   and restore; document rollback, local access and the selected
   image digest; then obtain owner approval for that exact
   commit **before** any tag, publication or rollout.

A live release needs a new authorized request. This document does
not authorize touching the household Docker services, PostgreSQL,
storage, CoreDNS, router, firewall, DNS records, real feed sources,
credentials, TLS certificate material, or image registries.

See [PW070-ACCEPTANCE.md](PW070-ACCEPTANCE.md),
[DEPLOYMENT.md](DEPLOYMENT.md), [BACKUP-RESTORE.md](BACKUP-RESTORE.md),
[RESOURCE-ENVELOPE.md](RESOURCE-ENVELOPE.md) and
[ROADMAP.md](ROADMAP.md).
