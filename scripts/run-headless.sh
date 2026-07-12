#!/usr/bin/env bash
# Run sregame under gamescope's headless backend: full GPU (Vulkan) rendering
# on a virtual Wayland display, with no physical display or compositor.
#
# This COMPLEMENTS the game's --headless flag rather than replacing it:
#
#   cargo run -- --headless      no window, no GPU, no frames - fast logic /
#                                telemetry smoke tests, works anywhere (CI)
#   scripts/run-headless.sh      real GPU-rendered frames on a virtual
#                                display - visual verification and BRP
#                                screenshots (pass -- --remote)
#
# Usage:
#   ./scripts/run-headless.sh [cargo args] -- [game args]
#
# Examples:
#   ./scripts/run-headless.sh                                # run the game
#   ./scripts/run-headless.sh --release -- --seconds 10      # timed run
#   ./scripts/run-headless.sh -- --remote --remote-port 15799 --seconds 60
#
# With --remote up, capture a frame via BRP (bevy_brp_extras):
#   curl -s -X POST http://127.0.0.1:15799/ \
#     -H 'Content-Type: application/json' \
#     -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/screenshot",
#          "params":{"path":"/tmp/sregame.png"}}'

set -euo pipefail

if ! command -v gamescope >/dev/null 2>&1; then
    echo "error: gamescope is not installed (Arch: sudo pacman -S gamescope)" >&2
    exit 1
fi

# Everything before -- goes to cargo, everything after goes to the game.
CARGO_ARGS=()
GAME_ARGS=()
PARSING_CARGO=true

for arg in "$@"; do
    if [ "$arg" = "--" ] && [ "$PARSING_CARGO" = true ]; then
        PARSING_CARGO=false
        continue
    fi
    if [ "$PARSING_CARGO" = true ]; then
        CARGO_ARGS+=("$arg")
    else
        GAME_ARGS+=("$arg")
    fi
done

echo "🎮 sregame under gamescope (headless backend, 1920x1080)" >&2

# -w/-h: game resolution, -W/-H: virtual output resolution.
# 'cargo run' from the repo root keeps asset paths working.
exec gamescope \
    --backend headless \
    -w 1920 -h 1080 \
    -W 1920 -H 1080 \
    -- cargo run "${CARGO_ARGS[@]}" -- "${GAME_ARGS[@]}"
