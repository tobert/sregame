#!/usr/bin/env bash
# Test headless execution with cage compositor
# Usage: ./scripts/test-headless-cage.sh [OPTIONS]
#   OPTIONS are passed to the game (e.g., --seconds 3 --remote)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Default timeout for the test
TIMEOUT=${HEADLESS_TIMEOUT:-15}
SECONDS_ARG=""
EXTRA_ARGS=()

# Parse arguments to extract --seconds if provided
for arg in "$@"; do
    if [[ "$arg" =~ ^--seconds ]]; then
        SECONDS_ARG="$arg"
    else
        EXTRA_ARGS+=("$arg")
    fi
done

# If no --seconds provided, default to 3
if [[ -z "$SECONDS_ARG" ]]; then
    SECONDS_ARG="--seconds 3"
fi

echo "🚀 Testing headless execution with cage..."
echo "   Timeout: ${TIMEOUT}s"
echo "   Game args: ${SECONDS_ARG} ${EXTRA_ARGS[*]:-}"
echo ""

# Run with timeout to prevent hanging forever
timeout "$TIMEOUT" cage -- cargo run -- $SECONDS_ARG "${EXTRA_ARGS[@]}" 2>&1 || {
    EXIT_CODE=$?
    if [ $EXIT_CODE -eq 124 ]; then
        echo ""
        echo "❌ Test timed out after ${TIMEOUT}s"
        echo "   The game may be hanging during initialization"
        exit 1
    else
        echo ""
        echo "✅ Test completed with exit code: $EXIT_CODE"
        exit 0
    fi
}

echo ""
echo "✅ Test completed successfully!"
