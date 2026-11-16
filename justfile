set shell := ["bash", "-uc"]

default:
	@just --list


tokei:
	tokei --exclude vendor --exclude tmp --files --sort lines

llvm-cov:
	cargo llvm-cov

llvm-lines:
	cargo llvm-lines

# Context
context:
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension rs . > ~/Downloads/all.txt

context-by-file:
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension rs . > ~/Downloads/rs.txt
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension md . > ~/Downloads/md.txt
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension toml . > ~/Downloads/toml.txt
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension yaml --extension yml . > ~/Downloads/yaml.txt
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension txt . > ~/Downloads/txt.txt
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension sh --extension bash . > ~/Downloads/sh.txt
	files-to-prompt --ignore tmp --ignore target/ --ignore .git/ --markdown --line-numbers --extension json . > ~/Downloads/json.txt

# PW-071 manual release safeguard.
# These legacy recipe names MUST NEVER execute cargo-release, commit, tag
# or push; inspect readiness only. The actual owner-approved release
# remains a separately reviewed manual operation.
release-status:
	@node scripts/pw071-release-preflight.mjs --status

major:
	@node scripts/pw071-release-preflight.mjs --enforce --type major
	@printf '%s\n' 'PW-071: major readiness declared; NO version bump, tag, publication, push or deployment was performed.'

minor:
	@node scripts/pw071-release-preflight.mjs --enforce --type minor
	@printf '%s\n' 'PW-071: minor readiness declared; NO version bump, tag, publication, push or deployment was performed.'

patch:
	@node scripts/pw071-release-preflight.mjs --enforce --type patch
	@printf '%s\n' 'PW-071: patch readiness declared; NO version bump, tag, publication, push or deployment was performed.'

# Copy over template files
template dir:
	cp -riv {{dir}}/docs .
	cp -iv {{dir}}/*.toml .
	cp -iv {{dir}}/about.hbs .
	cp -iv {{dir}}/biome.json .
	cp -iv {{dir}}/.gitignore .
	cp -iv {{dir}}/LICENSE .

# Build
build:
	cargo build 

# Format (write/fix)
fmt:
	cargo fmt
	taplo fmt
	biome check --write .
	rumdl fmt .

# Validate (configs)
validate:
	taplo validate

# Spellcheck / Links
typos:
	typos --config typos.toml

links:
	lychee --config lychee.toml .

# Lint
clippy:
	cargo clippy --all-targets -- -D warnings

biome:
	biome check .

rumdl:
	rumdl check .

# Test
test:
	cargo test --all-features

# Docs
doc:
	cargo doc --no-deps

# Data
sync-normalization:
	scripts/sync_normalization_any.sh

refresh-format-fixtures:
	CITE_OTTER_CORE_LIMIT=200 cargo run --quiet --bin generate_format_fixtures

refresh-fixtures:
	scripts/sync_normalization_any.sh
	CITE_OTTER_CORE_LIMIT=200 cargo run --quiet --bin generate_format_fixtures
	cargo test

refresh-fixtures-fast:
	scripts/sync_normalization_any.sh
	CITE_OTTER_CORE_LIMIT=200 cargo run --quiet --bin generate_format_fixtures

compare-ruby-format:
	scripts/compare_ruby_format.sh

bench-ruby-parity:
	scripts/benchmark_ruby_parity.sh

bench-rust-baseline:
	scripts/benchmark_rust_baseline.sh

# Security (Rust)
audit:
	cargo audit

deny:
	cargo deny check

# Coverage (Rust)
# (Requires cargo-llvm-cov installed)
coverage:
	cargo llvm-cov --all-features --lcov --output-path target/coverage.lcov

coverage-html:
	cargo llvm-cov --all-features --open

# CI-style checks (no writes)
fmt-check:
	cargo fmt --check
	taplo fmt --check
	biome check .
	rumdl check .

# "CI local" = what you'd run before pushing
ci: fmt-check validate typos links biome clippy test doc build

# "All" = auto-fix + run the suite
all: fmt validate typos links biome clippy test doc build
