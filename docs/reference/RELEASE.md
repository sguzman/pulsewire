# Release & SemVer Policy

This repo uses Conventional Commits and git-cliff to generate changelogs, with
cargo-release for coordinated version bumps.

## Commit Conventions

Use Conventional Commits:

- `feat:` new user-facing functionality
- `fix:` bug fixes
- `docs:` documentation changes
- `refactor:` internal refactors without behavior change
- `perf:` performance improvements
- `test:` test changes
- `chore:` maintenance tasks
- `build:` build/tooling changes
- `ci:` CI pipeline changes

Add `!` for breaking changes (e.g., `feat!: ...`).

## Versioning Policy

- Stay in `0.x` while APIs and config schemas are still evolving.
- Promote to `1.0.0` once server API + config schemas are stable and at least
  one client ships.

## Changelog

Generate the changelog with git-cliff:

```bash
git cliff --config cliff.toml --output CHANGELOG.md
```

## Release flow (manual; blocked pending PW-071)

**No tag, publication, version bump, container rollout or push is authorized
by successful documentation or CI checks.** First verify all applicable
P0/P1 gates in [Pulsewire's roadmap](../pulsewire/ROADMAP.md), including
trusted ingress/TLS, actual-host resource measurements, backup/recovery
drills and a recorded owner approval for the exact release commit.

For a future explicitly approved release:

1. Review the proposed version, breaking compatibility changes, API/schema
   and configuration migration, rollback plan, and retained backups.
2. Create a reviewable changelog/version change with `cargo-release` and
   `git-cliff` **in a rehearsal/PR**, not by running an auto-tag/push
   recipe. Confirm the result against the exact tested source commit.
3. After the owner approves the release commit, execute the reviewed
   version/tag steps and only then the separately approved publication or
   deployment. Record the tag, image digest, backup ID and acceptance result.

**Historical hazard fixed:** the root `justfile` formerly let
`major`, `minor`, and `patch` invoke `cargo release --execute`,
`git push`, and `git push --tags` automatically. They now call a
**fail-closed, read-only PW-071 gate** and, even if future approval
conditions are met, **do not** execute a release or change any state.
The safe `just release-status` or direct
`node scripts/pw071-release-preflight.mjs --status` prints a
non-authorizing checklist. See the
[PW-071 release preflight](../pulsewire/RELEASE-PREFLIGHT.md).
No `cargo release` invocation is required to check readiness.
An actual future release requires the owner to approve and perform
**separate manually reviewed** version, tag and publication operations
against an exact accepted commit.

Notes:

- `publish = false` in `release.toml` so crates are not published.
- `pre-release-hook` runs git-cliff to generate CHANGELOG.md for every release.
- `cliff.toml` expects `[remote.github]` `owner`/ `repo` to be set (and the
  issue-link replace URL updated) so changelog links point to the right project
  before running `git cliff`.

## Files

- `cliff.toml` – git-cliff config
- `release.toml` – cargo-release config
- `CHANGELOG.md` – generated changelog (optional until first release)
