# Bevy 0.17 Headless Mode Guide

## Overview

This game supports **true headless mode** - running without any window, GPU, or display server. This is perfect for:

- **CI/CD pipelines** - Automated testing in Docker containers
- **Remote servers** - Run game logic on servers without displays
- **Automated testing** - Test game behavior programmatically via BRP
- **Observability testing** - Verify telemetry integration without GUI

## How It Works

### The Configuration

```rust
if args.headless {
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,  // ← Key: No window = no surface creation
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>()  // ← Disable windowing system integration
            .set(ImagePlugin::default_nearest())
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(
        Duration::from_secs_f64(1.0 / 60.0)  // ← 60 FPS update loop
    ));
}
```

### What Runs in Headless Mode

✅ **Runs:**
- ECS Update loop (60 FPS by default)
- All game systems (player, NPCs, dialogue, state management)
- Asset loading (textures, sprites, maps)
- Bevy Remote Protocol (BRP) server
- OpenTelemetry telemetry export
- Event processing
- Transform updates
- Physics/collision (if present)

❌ **Doesn't Run:**
- Window creation
- GPU surface creation
- Frame presentation
- Window event handling (keyboard/mouse from window)
- Visual rendering to screen

### Architecture Details

When `primary_window: None`:
1. **RenderPlugin** still initializes but skips surface creation
2. **No ERROR_SURFACE_LOST_KHR panic** - surfaces never created
3. **Render graph** exists but doesn't present frames
4. **Assets** load normally (useful for BRP queries)
5. **ScheduleRunnerPlugin** drives the main loop instead of winit

## Usage

### Basic Headless Mode

```bash
# Run for 100 frames
cargo run -- --headless --frames 100

# Run for 30 seconds
cargo run -- --headless --seconds 30

# Run indefinitely (Ctrl+C to stop)
cargo run -- --headless
```

### Headless + BRP (Remote Control)

```bash
# Start with BRP enabled
cargo run -- --headless --remote --seconds 60

# In another terminal, query game state:
curl -X POST http://127.0.0.1:15702 \
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
  }' | jq '.'
```

### Headless + OpenTelemetry

```bash
# Start with OTLP telemetry
cargo run -- --headless --remote \
  --otlp-endpoint 127.0.0.1:4317 \
  --seconds 60

# Or use environment variable
OTEL_EXPORTER_OTLP_ENDPOINT=127.0.0.1:4317 \
  cargo run -- --headless --remote --seconds 60
```

### All Features Combined

```bash
# Full observability stack
cargo run -- \
  --headless \
  --remote \
  --otlp-endpoint 127.0.0.1:4317 \
  --frames 1000
```

## Bevy Remote Protocol (BRP)

BRP provides JSON-RPC access to the running game's ECS.

### Available Methods

#### Query Entities

```bash
# Get all entities with Transform
curl -X POST http://127.0.0.1:15702 \
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
  }'

# Query specific component types
curl -X POST http://127.0.0.1:15702 \
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
  }'
```

#### Get Component Data

```bash
curl -X POST http://127.0.0.1:15702 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "bevy/get",
    "params": {
      "entity": 0,
      "components": [
        "bevy_transform::components::transform::Transform"
      ]
    }
  }'
```

### BRP Server Details

- **Host:** `127.0.0.1` (localhost only)
- **Port:** `15702` (Bevy default)
- **Protocol:** JSON-RPC 2.0 over HTTP
- **Content-Type:** `application/json`

## Testing & CI/CD

### Docker Example

```dockerfile
FROM rust:1.80

# No X11, Wayland, or GPU required!
RUN apt-get update && apt-get install -y \
    libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Run headless tests
RUN cargo test
RUN cargo run -- --headless --frames 100
```

### GitHub Actions Example

```yaml
name: Headless Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable

      # Run headless tests - no display server needed!
      - name: Run headless mode
        run: cargo run -- --headless --frames 100

      - name: Test BRP integration
        run: |
          # Start game in background
          cargo run -- --headless --remote --seconds 30 &
          sleep 3

          # Test BRP connectivity
          curl -X POST http://127.0.0.1:15702 \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","id":1,"method":"bevy/query","params":{"data":{"components":["bevy_transform::components::transform::Transform"],"option":"all"}}}'
```

### Integration Test Example

```rust
#[test]
fn test_game_in_headless_mode() {
    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>()
    )
    .add_plugins(ScheduleRunnerPlugin::run_once());

    // Add your game plugins
    app.add_plugins(GameStatePlugin);

    // Run the app for one frame
    app.run();

    // Assert game state is correct
}
```

## Troubleshooting

### Audio Errors (Safe to Ignore)

```
ALSA lib pcm_dmix.c:1000:(snd_pcm_dmix_open) unable to open slave
```

This is normal in headless mode - audio initialization tries to connect but gracefully handles failure.

### BRP Not Responding

1. **Check the `--remote` flag is set**
   ```bash
   cargo run -- --headless --remote
   ```

2. **Verify BRP port is not in use**
   ```bash
   netstat -tulpn | grep 15702
   ```

3. **Wait for initialization**
   BRP server starts after ~2-3 seconds. Add `sleep 3` before querying.

### Missing Telemetry Resources

If you see panics about `Res<GameTracer>` or `Res<GameMeter>`:

**Wrong:**
```rust
fn my_system(tracer: Res<GameTracer>) {
    // Panics when telemetry disabled
}
```

**Correct:**
```rust
fn my_system(tracer: Option<Res<GameTracer>>) {
    if let Some(tracer) = tracer {
        // Use tracer
    }
}
```

## Performance

Headless mode is **faster** than windowed mode:

- **No vsync** - runs at full speed (capped by ScheduleRunnerPlugin)
- **No rendering overhead** - GPU not involved
- **Lower memory** - no framebuffers allocated
- **Consistent timing** - ScheduleRunnerPlugin provides stable frame timing

Typical performance:
- **Windowed:** ~60 FPS (vsync limited)
- **Headless:** 60 FPS (ScheduleRunnerPlugin limited, can go higher)
- **CPU usage:** 30-50% lower in headless mode

## Screenshots in Headless Mode

Currently **not supported** - requires render target creation which conflicts with headless mode.

For screenshot capability, you would need:
1. Software rendering backend (like wgpu's Vulkan/CPU adapter)
2. Render to texture instead of window surface
3. Custom screenshot capture system

This is a potential future enhancement but not critical for testing/observability.

## Related Files

- `/home/atobey/src/sregame/src/main.rs` - Headless mode configuration
- `/home/atobey/src/sregame/examples/headless_brp_demo.sh` - Demo script
- Bevy source: `/home/atobey/src/bevy/crates/bevy_internal/src/default_plugins.rs`

## References

- [Bevy Remote Protocol Docs](https://docs.rs/bevy_remote/)
- [Bevy WindowPlugin Docs](https://docs.rs/bevy/latest/bevy/window/struct.WindowPlugin.html)
- [ScheduleRunnerPlugin Docs](https://docs.rs/bevy/latest/bevy/app/struct.ScheduleRunnerPlugin.html)
- Original implementation: Commit 88e6aec (feat: Headless mode implementation for Bevy 0.17)
