set shell := ["bash", "-uc"]

default:
	@just --list

# Build
build:
	cargo build

# Format
fmt:
	cargo fmt
	taplo fmt
	biome format --write .

# Validate
links:
	lychee --config lychee.toml .
validate:
	taplo validate

# Test
test:
	cargo test

# Post-change checklist
post-change:
	scripts/post-change.sh

# Everything
all: build post-change
