#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
V2_ROOT="$PWD"
TASK_RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export RUSTUP_HOME="$TASK_RUSTUP_HOME"
mkdir -p .work/home .work/tmp
export HOME="$V2_ROOT/.work/home" TMPDIR="$V2_ROOT/.work/tmp" XDG_CACHE_HOME="$V2_ROOT/.work/cache" CARGO_HOME="$V2_ROOT/.work/cargo-home"
# Explicitly preserve the real toolchain path when HOME is synthetic.
node qa/oracle.mjs
node qa/host_checks.mjs
.work/python/bin/python qa/schema.py
npm --prefix .work/v1 run typecheck
npm --prefix .work/v1 test
for toolchain in stable 1.85.0; do
 cargo +"$toolchain" fmt --all -- --check
 cargo +"$toolchain" test --locked --all-targets
 cargo +"$toolchain" clippy --locked --all-targets -- -D warnings
done
cargo build --locked --example fixture
python3 qa/pipes.py target/debug/examples/fixture
python3 scripts/verify.py
