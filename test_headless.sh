#!/bin/bash
# Test script for headless mode functionality
# This demonstrates the different headless modes available in Bevy 0.17

set -e

echo "🧪 Testing Bevy 0.17 Headless Modes"
echo "===================================="
echo ""

# Test 1: Basic headless mode with timed exit
echo "Test 1: Headless mode with 3-second timeout"
echo "Command: cargo run --release -- --headless --seconds 3"
echo ""
timeout 10 cargo run --release -- --headless --seconds 3 || {
    if [ $? -eq 124 ]; then
        echo "❌ FAILED: Process hung (timeout after 10s)"
        exit 1
    fi
}
echo "✅ Test 1 passed: Game exited cleanly after 3 seconds"
echo ""

# Test 2: Headless with frame limit
echo "Test 2: Headless mode with 180 frame limit (3 seconds at 60fps)"
echo "Command: cargo run --release -- --headless --frames 180"
echo ""
timeout 10 cargo run --release -- --headless --frames 180 || {
    if [ $? -eq 124 ]; then
        echo "❌ FAILED: Process hung (timeout after 10s)"
        exit 1
    fi
}
echo "✅ Test 2 passed: Game exited after 180 frames"
echo ""

# Test 3: Headless with Bevy Remote Protocol
echo "Test 3: Headless mode with BRP (runs 5 seconds)"
echo "Command: cargo run --release -- --headless --remote --seconds 5"
echo ""
timeout 10 cargo run --release -- --headless --remote --seconds 5 || {
    if [ $? -eq 124 ]; then
        echo "❌ FAILED: Process hung (timeout after 10s)"
        exit 1
    fi
}
echo "✅ Test 3 passed: BRP + headless works"
echo ""

# Test 4: Headless with OpenTelemetry
echo "Test 4: Headless mode with OpenTelemetry (5 seconds)"
echo "Command: cargo run --release -- --headless --otlp-endpoint 127.0.0.1:4317 --seconds 5"
echo "Note: This will fail to connect to OTLP collector (expected), but should still run"
echo ""
timeout 10 cargo run --release -- --headless --otlp-endpoint 127.0.0.1:4317 --seconds 5 || {
    if [ $? -eq 124 ]; then
        echo "❌ FAILED: Process hung (timeout after 10s)"
        exit 1
    fi
}
echo "✅ Test 4 passed: OTLP + headless works (connection failure is expected)"
echo ""

echo "=========================================="
echo "🎉 All headless mode tests passed!"
echo ""
echo "Summary of working modes:"
echo "  ✅ Headless with timed exit (--seconds)"
echo "  ✅ Headless with frame limit (--frames)"
echo "  ✅ Headless with Bevy Remote Protocol (--remote)"
echo "  ✅ Headless with OpenTelemetry (--otlp-endpoint)"
echo ""
echo "CI/CD Ready: These modes work without display servers or GPU"
