#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
V2_ROOT="$PWD"
TASK_RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export RUSTUP_HOME="$TASK_RUSTUP_HOME"
mkdir -p .work/home .work/tmp .work/v1
export HOME="$V2_ROOT/.work/home" TMPDIR="$V2_ROOT/.work/tmp" XDG_CACHE_HOME="$V2_ROOT/.work/cache" CARGO_HOME="$V2_ROOT/.work/cargo-home"
# Exact tracked V1 root/lock; no source-root npm mutation and no lifecycle scripts.
git -C .. archive 48b1cd483b6e8fbf2d8ae8394ffdb11b4e8f659c | tar -x -C .work/v1
npm ci --prefix .work/v1 --cache .work/npm-cache --ignore-scripts --legacy-peer-deps --no-audit --no-fund
python3 -m venv .work/python
.work/python/bin/pip install --disable-pip-version-check --cache-dir .work/pip-cache -r qa/requirements.txt
cargo fetch --locked
