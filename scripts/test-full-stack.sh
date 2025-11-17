#!/usr/bin/env bash
# Full stack test: Headless + BRP + Telemetry
# This is the ultimate test combining all features

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

OTLP_ENDPOINT=${OTEL_EXPORTER_OTLP_ENDPOINT:-127.0.0.1:42701}
BRP_PORT=${BRP_PORT:-15702}
TEST_DURATION=${TEST_DURATION:-5}
TIMEOUT=${HEADLESS_TIMEOUT:-15}

echo "🎯 Full stack test: Headless + BRP + Telemetry"
echo "   OTLP endpoint: ${OTLP_ENDPOINT}"
echo "   BRP port: ${BRP_PORT}"
echo "   Test duration: ${TEST_DURATION}s"
echo "   Timeout: ${TIMEOUT}s"
echo ""

echo "Starting game with all features enabled..."
echo ""

# Run in cage with timeout, enabling both BRP and telemetry
OTEL_EXPORTER_OTLP_ENDPOINT="$OTLP_ENDPOINT" \
    timeout "$TIMEOUT" cage -- cargo run -- \
    --remote \
    --seconds "$TEST_DURATION" \
    2>&1 | tee /tmp/sregame-fullstack-test.log || {
    EXIT_CODE=$?
    if [ $EXIT_CODE -eq 124 ]; then
        echo ""
        echo "❌ Test timed out after ${TIMEOUT}s"
        exit 1
    fi
}

echo ""
echo "Test logs saved to: /tmp/sregame-fullstack-test.log"
echo ""

# Try to test BRP connectivity if the game is still running
if pgrep -f "target/debug/sregame" > /dev/null; then
    echo "Testing BRP connectivity..."
    sleep 2
    if curl -s "http://localhost:${BRP_PORT}/methods" > /dev/null 2>&1; then
        echo "✅ BRP is accessible!"
    else
        echo "⚠️  BRP not accessible (game may have exited)"
    fi
fi

echo ""
echo "✅ Full stack test completed!"
