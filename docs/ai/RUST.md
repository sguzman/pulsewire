# AI Rules for This Rust Repo

These rules apply to any AI agent making changes in this repository.

## Required workflow

### 1) Build must succeed (required)

After making changes, ensure the repo builds:

- Run: `cargo build`

If the build fails, fix the issues until it compiles.

### 2) Provide a commit message only (required)

After completing a coherent, working change:

- Generate an appropriate commit message
- Do not run git commands (add/commit/push); the user will do that manually

### 3) Post-change checks are manual (required)

All formatting, linting, validation, and test commands are run manually by the user.
See `docs/ai/POST-CHANGES.md` for the full checklist.

### 4) Refactor oversized Rust files into modules (required)

If any Rust source file grows beyond 600 lines, refactor it into modules.

- Convert the file into a directory module:
  - Example: `handler.rs` becomes `handler/`
- Split the code into multiple files inside the new directory, organized by clear functional/domain boundaries.
  - Prefer cohesive modules with single responsibilities (e.g., parsing, validation, IO, DB, HTTP, types, errors, helpers).
- Add `handler/mod.rs` that re-exports and wires the modules so other code can keep importing through the parent module.
  - Keep public surface area intentional: export only what needs to be used externally.
- Preserve behavior and public API where possible:
  - Avoid churn in call sites unless there is a strong reason.
  - If imports/paths must change, update them consistently across the repo.
- After refactoring, re-run step 1 (build) to ensure everything still compiles.
