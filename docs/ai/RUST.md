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

### 4) Update docs related to changes (required)

Before committing, update any documentation impacted by the change:

- Update relevant docs (README, docs/, inline module docs, examples, CHANGELOG notes if present, etc.)
- Ensure docs match the actual behavior, CLI flags/config keys, APIs, and file paths introduced/changed
- If no doc updates are needed, explicitly state: "Docs: no changes needed"

### 5) Commit and push after a working modification (required)

After completing a coherent, working change:

- Generate an appropriate commit message
- Commit the changes
- Push to the configured remote

### 6) Refactor oversized Rust files into modules (required)

If any Rust source file grows beyond 500 lines, refactor it into modules.

- Convert the file into a directory module:
  - Example: `handler.rs` becomes `handler/`
- Split the code into multiple files inside the new directory, organized by clear functional/domain boundaries.
  - Prefer cohesive modules with single responsibilities (e.g., parsing, validation, IO, DB, HTTP, types, errors, helpers).
- Add `handler/mod.rs` that re-exports and wires the modules so other code can keep importing through the parent module.
  - Keep public surface area intentional: export only what needs to be used externally.
- Preserve behavior and public API where possible:
  - Avoid churn in call sites unless there is a strong reason.
  - If imports/paths must change, update them consistently across the repo.
- After refactoring, re-run steps 1-3 (build/tests/taplo) to ensure everything still compiles and behaves correctly.
