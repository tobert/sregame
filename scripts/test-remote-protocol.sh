#!/usr/bin/env bash
# Test Bevy Remote Protocol (BRP) functionality
# Starts the game with BRP enabled and tests basic connectivity

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

BRP_PORT=${BRP_PORT:-15702}  # Default Bevy Remote Protocol port
TEST_DURATION=${TEST_DURATION:-5}

echo "🔌 Testing Bevy Remote Protocol..."
echo "   Port: ${BRP_PORT}"
echo "   Test duration: ${TEST_DURATION}s"
echo ""

# Start the game with BRP enabled in the background
echo "Starting game with --remote flag..."
cargo run -- --remote --seconds "$TEST_DURATION" &
GAME_PID=$!

# Give the game time to start and initialize BRP
echo "Waiting for BRP to initialize..."
sleep 3

# Test BRP connectivity
echo "Testing BRP connectivity on port ${BRP_PORT}..."
if curl -s "http://localhost:${BRP_PORT}/methods" > /dev/null 2>&1; then
    echo "✅ BRP is accessible!"
    echo ""
    echo "Available methods:"
    curl -s "http://localhost:${BRP_PORT}/methods" | jq '.' || echo "(jq not available)"
else
    echo "❌ BRP is not accessible on port ${BRP_PORT}"
    echo "   Make sure the game is running with --remote flag"
fi

# Wait for the game to exit
echo ""
echo "Waiting for game to exit..."
wait $GAME_PID || {
    EXIT_CODE=$?
    echo "Game exited with code: $EXIT_CODE"
}

echo ""
echo "✅ Test completed!"
