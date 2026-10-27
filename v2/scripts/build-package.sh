#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
V2_ROOT="$PWD"
TASK_RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
mkdir -p .work/home .work/tmp
TASK_BUILD_CARGO_HOME="$(mktemp -d "$V2_ROOT/.work/production-cargo-XXXXXX")"
TASK_BUILD_TARGET="$(mktemp -d "$V2_ROOT/.work/production-target-XXXXXX")"
export RUSTUP_HOME="$TASK_RUSTUP_HOME" HOME="$V2_ROOT/.work/home" TMPDIR="$V2_ROOT/.work/tmp" CARGO_HOME="$TASK_BUILD_CARGO_HOME" CARGO_TARGET_DIR="$TASK_BUILD_TARGET"
cargo +stable build --locked --release --bin ip-geolocation
python3 scripts/package.py
python3 qa/production_smoke.py
