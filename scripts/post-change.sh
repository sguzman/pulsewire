#!/usr/bin/env bash
set -euo pipefail

cargo fmt
taplo fmt
taplo validate
biome format --write .
lychee --config lychee.toml .
cargo test
