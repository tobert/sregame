#!/usr/bin/env bash
# Test game with OpenTelemetry OTLP telemetry enabled
# Requires otlp-mcp to be running

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

OTLP_ENDPOINT=${OTEL_EXPORTER_OTLP_ENDPOINT:-127.0.0.1:42701}
TEST_DURATION=${TEST_DURATION:-5}

echo "📊 Testing game with OpenTelemetry telemetry..."
echo "   OTLP endpoint: ${OTLP_ENDPOINT}"
echo "   Test duration: ${TEST_DURATION}s"
echo ""

# Check if otlp-mcp is configured
if command -v claude-code &> /dev/null; then
    echo "✅ Claude Code detected - otlp-mcp should be available"
else
    echo "⚠️  Claude Code not detected - telemetry may not work"
fi

echo ""
echo "Starting game with telemetry..."
OTEL_EXPORTER_OTLP_ENDPOINT="$OTLP_ENDPOINT" \
    cargo run -- --seconds "$TEST_DURATION" 2>&1 | tee /tmp/sregame-telemetry-test.log

echo ""
echo "✅ Test completed!"
echo "   Check logs in: /tmp/sregame-telemetry-test.log"
echo ""
echo "To query telemetry data, use the otlp-mcp tools in Claude Code:"
echo "  - mcp__otlpmcp__get_stats"
echo "  - mcp__otlpmcp__create_snapshot"
echo "  - mcp__otlpmcp__get_snapshot_data"
