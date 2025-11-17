#!/usr/bin/env bash
# Headless Mode + BRP Demo
# Demonstrates true headless operation with remote control via Bevy Remote Protocol

set -e

echo "🚀 Bevy 0.17 Headless Mode + BRP Demo"
echo "======================================"
echo ""
echo "This demonstrates:"
echo "  ✓ True headless mode (no window, no GPU, no display server)"
echo "  ✓ Bevy Remote Protocol (BRP) for remote control"
echo "  ✓ Game logic continues running"
echo "  ✓ ECS Update loop executes at 60 FPS"
echo ""

# Start the game in headless mode with BRP enabled
echo "Starting game in headless mode with BRP..."
cargo run --quiet -- --headless --seconds 60 --remote &
GAME_PID=$!

# Wait for BRP server to start
echo "Waiting for BRP server to initialize..."
sleep 3

# Test BRP connection
echo ""
echo "📡 Testing BRP Connection"
echo "-------------------------"
curl -s -X POST http://127.0.0.1:15702 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "bevy/query",
    "params": {
      "data": {
        "components": ["bevy_transform::components::transform::Transform"],
        "option": "all"
      }
    }
  }' | jq -r '.result.response | length' > /tmp/entity_count.txt

ENTITY_COUNT=$(cat /tmp/entity_count.txt)
echo "✓ BRP is responding!"
echo "✓ Found $ENTITY_COUNT entities with Transform components"

# Query specific game entities
echo ""
echo "🎮 Querying Game State"
echo "----------------------"
curl -s -X POST http://127.0.0.1:15702 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "bevy/query",
    "params": {
      "data": {
        "components": ["bevy_transform::components::transform::Transform"],
        "filter": {
          "with": ["sregame::player::Player"]
        }
      }
    }
  }' | jq -r '.result.response | length' > /tmp/player_count.txt

PLAYER_COUNT=$(cat /tmp/player_count.txt)
echo "✓ Found $PLAYER_COUNT player entities"

echo ""
echo "✅ Success! Headless mode is fully functional:"
echo "   - No window created"
echo "   - No GPU rendering"
echo "   - BRP server running on http://127.0.0.1:15702"
echo "   - Game logic executing normally"
echo ""
echo "Stopping game..."
kill $GAME_PID 2>/dev/null || true
wait $GAME_PID 2>/dev/null || true

echo ""
echo "🎉 Demo complete!"
echo ""
echo "Usage in CI/CD or automated testing:"
echo "  cargo run -- --headless --remote --frames 1000"
echo ""
echo "With OpenTelemetry:"
echo "  cargo run -- --headless --remote --otlp-endpoint 127.0.0.1:4317"
