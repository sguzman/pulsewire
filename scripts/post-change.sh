#!/usr/bin/env bash
set -euo pipefail

cargo fmt
taplo fmt
taplo validate
biome format --write .
lychee --config lychee.toml .
typos --config typos.toml
cargo test
