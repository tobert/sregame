#!/usr/bin/env bash
# Simpler Weston test that directly invokes the game as Weston's client
# This approach is often more reliable

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

WIDTH=${WESTON_WIDTH:-1920}
HEIGHT=${WESTON_HEIGHT:-1080}
EXTRA_ARGS=("$@")

echo "🎮 Running game with Weston headless backend..."
echo "   Resolution: ${WIDTH}x${HEIGHT}"
echo "   Args: ${EXTRA_ARGS[*]:-none}"
echo ""

# Run Weston with our game as the startup client
# Weston will automatically exit when the client exits
exec weston \
    --backend=headless-backend.so \
    --width=$WIDTH \
    --height=$HEIGHT \
    --use-pixman \
    --no-config \
    -- cargo run -- "${EXTRA_ARGS[@]}"
