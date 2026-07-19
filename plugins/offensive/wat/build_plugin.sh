#!/usr/bin/env bash
# build_plugin.sh — Build the offensive-security WASM plugin.
#
# Steps:
#   1. Compile offensive.wat -> offensive.wasm using /tmp/watc
#   2. Inject manifest.json as a 'slpx_manifest' custom section
#   3. Output the final plugin at ../offensive.wasm
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WAT_BIN="/tmp/watc/target/release/watc"

if [[ ! -x "$WAT_BIN" ]]; then
    echo "ERROR: WAT->WASM compiler not found at $WAT_BIN" >&2
    echo "Build it first:  cd /tmp/watc && cargo build --release" >&2
    exit 1
fi

cd "$HERE"

# 1. WAT -> WASM
"$WAT_BIN" offensive.wat offensive.wasm

# 2. Inject manifest (use the file from one level up)
python3 inject_manifest.py offensive.wasm ../manifest.json offensive.manifested.wasm

# 3. Place final plugin at ../offensive.wasm
cp offensive.manifested.wasm ../offensive.wasm

echo ""
echo "Plugin built:  ../offensive.wasm"
ls -la ../offensive.wasm

