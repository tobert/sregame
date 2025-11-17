#!/usr/bin/env bash
# Test headless execution with Weston's headless backend
# This is often more reliable than cage for true headless scenarios

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Configuration
WIDTH=${WESTON_WIDTH:-1920}
HEIGHT=${WESTON_HEIGHT:-1080}
TEST_DURATION=${TEST_DURATION:-5}
TIMEOUT=${HEADLESS_TIMEOUT:-15}

# Weston config
WESTON_CONFIG=$(mktemp)
trap "rm -f $WESTON_CONFIG" EXIT

cat > "$WESTON_CONFIG" << EOF
[core]
backend=headless-backend.so
use-pixman=true

[output]
name=headless
mode=${WIDTH}x${HEIGHT}

[keyboard]
keymap_rules=evdev

[shell]
background-color=0xff000000
EOF

echo "🖥️  Testing headless execution with Weston..."
echo "   Resolution: ${WIDTH}x${HEIGHT}"
echo "   Config: $WESTON_CONFIG"
echo "   Test duration: ${TEST_DURATION}s"
echo "   Timeout: ${TIMEOUT}s"
echo ""

# Start Weston in headless mode
echo "Starting Weston headless compositor..."
weston --backend=headless-backend.so \
       --width=$WIDTH \
       --height=$HEIGHT \
       --use-pixman \
       --no-config &
WESTON_PID=$!

# Give Weston time to start
sleep 2

# Check if Weston started successfully
if ! ps -p $WESTON_PID > /dev/null; then
    echo "❌ Weston failed to start"
    exit 1
fi

echo "✅ Weston running (PID: $WESTON_PID)"
echo ""

# Get the Wayland socket
WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-wayland-0}
export WAYLAND_DISPLAY

echo "Starting game (WAYLAND_DISPLAY=$WAYLAND_DISPLAY)..."
timeout "$TIMEOUT" cargo run -- --seconds "$TEST_DURATION" 2>&1 | tee /tmp/sregame-weston-test.log || {
    EXIT_CODE=$?
    if [ $EXIT_CODE -eq 124 ]; then
        echo ""
        echo "❌ Test timed out after ${TIMEOUT}s"
        kill $WESTON_PID 2>/dev/null || true
        exit 1
    fi
}

# Cleanup
echo ""
echo "Stopping Weston..."
kill $WESTON_PID 2>/dev/null || true
wait $WESTON_PID 2>/dev/null || true

echo ""
echo "✅ Test completed!"
echo "   Logs saved to: /tmp/sregame-weston-test.log"
