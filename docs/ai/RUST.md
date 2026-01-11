# AI Rules for This Rust Repo

These rules apply to any AI agent making changes in this repository.

## Required workflow

### 1) Build must succeed (required)
After making changes, ensure the repo builds:

- Run: `cargo build`

Do not leave the repo in a state where it no longer compiles.

### 2) Tests must pass when relevant (required)
If your change could affect behavior (most code changes do), run tests:

- Run: `cargo test`

If tests are not runnable in the current environment (missing deps, platform limits, etc.), clearly state what was attempted and what blocked it.

### 3) Configuration must be valid (required)
If you edit TOML (Cargo.toml, workspace config, tool configs, etc.), validate it:

- Run: `taplo validate`

If additional validators are requested later (formatters, linters, schema checks, nix checks, etc.), treat them as required for the relevant files and run them before committing.

### 4) Commit and push after a working modification (required)
After completing a coherent, working change:

- Generate an appropriate commit message
- Commit the changes
- Push to the configured remote
