#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
echo "=== AEGIS Engine Release Build ==="

echo "[1/3] Building frontend..."
cd frontend && npm install && npm run build && cd ..

echo "[2/3] Compiling engine (release)..."
cd engine && cargo build --release && cd ..

# Make binary extension conditional on OS
ENGINE_BIN="engine/target/release/aegis-engine"
if [ "$(uname -s)" = "MINGW*" ] || [ "$(uname -s)" = "MSYS*" ] || [ "$OSTYPE" = "msys" ] || [ "$OSTYPE" = "cygwin" ]; then
    ENGINE_BIN="${ENGINE_BIN}.exe"
fi

if [ -f "$ENGINE_BIN" ]; then
    if [ "$(uname -s)" = "Darwin" ]; then
        SIZE=$(stat -f%z "$ENGINE_BIN" 2>/dev/null)
    else
        SIZE=$(stat --format=%s "$ENGINE_BIN" 2>/dev/null)
    fi
    echo "[3/3] Binary: $ENGINE_BIN ($SIZE bytes)"
fi
echo "=== Engine release build complete ==="
