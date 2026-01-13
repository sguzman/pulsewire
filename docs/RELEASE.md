# Release & SemVer Policy

This repo uses Conventional Commits and git-cliff to generate changelogs, with cargo-release for coordinated version bumps.

## Commit conventions

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

## Versioning policy

- Stay in `0.x` while APIs and config schemas are still evolving.
- Promote to `1.0.0` once server API + config schemas are stable and at least one client ships.

## Changelog

Generate the changelog with git-cliff:

```
git cliff --config cliff.toml --output CHANGELOG.md
```

## Release flow (manual)

1) Update version with cargo-release (workspace)
2) Generate changelog
3) Tag the release (no publish)

Example:

```
cargo release patch --workspace
```

Notes:
- `publish = false` in `release.toml` so crates are not published.

## Files

- `cliff.toml` – git-cliff config
- `release.toml` – cargo-release config
- `CHANGELOG.md` – generated changelog (optional until first release)
