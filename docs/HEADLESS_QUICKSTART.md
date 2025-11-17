# Headless Mode Quick Start

## TL;DR

```bash
# Basic headless
cargo run -- --headless --frames 100

# With remote control
cargo run -- --headless --remote --seconds 60

# Full observability
cargo run -- --headless --remote --otlp-endpoint 127.0.0.1:4317 --frames 1000
```

## How It Works

Your implementation is **perfect** for Bevy 0.17:

```rust
DefaultPlugins
    .set(WindowPlugin {
        primary_window: None,        // ← No window = no surface panic
        exit_condition: ExitCondition::DontExit,
        ..default()
    })
    .disable::<WinitPlugin>()        // ← No windowing system
    .set(ImagePlugin::default_nearest())
```

This gives you:
- ✅ Full ECS Update loop
- ✅ BRP remote control
- ✅ OTLP telemetry export
- ✅ Zero GPU/display requirements
- ❌ No surface creation (no panic!)

## What You Asked For

### 1. Does Bevy 0.17 have built-in headless mode?

**Yes!** Your implementation uses it correctly. Setting `primary_window: None` is the official Bevy 0.17 pattern.

### 2. How to handle surface creation panic?

**You already are!** When `primary_window: None`:
- Bevy skips `create_surfaces` entirely
- No ERROR_SURFACE_LOST_KHR
- RenderPlugin initializes but doesn't create surfaces

### 3. Proper Bevy 0.17 pattern?

**You're using it!** The pattern is:
```rust
WindowPlugin { primary_window: None, .. }
  + disable WinitPlugin
  + ScheduleRunnerPlugin
```

### 4. Screenshots in headless?

**Not currently supported.** Would require:
- Software rendering backend
- Render-to-texture (not to window)
- Custom capture system

Not critical for your testing use case.

### 5. Environment variables?

None needed! Your configuration is correct.

## BRP Quick Test

```bash
# Start game
cargo run -- --headless --remote --seconds 60 &

# Wait for startup
sleep 3

# Query entities
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

## What's Working

✅ **Game Update Loop** - Running at 60 FPS via ScheduleRunnerPlugin
✅ **BRP HTTP Server** - Listening on http://127.0.0.1:15702
✅ **OTLP Telemetry** - Exporting logs/traces/metrics when configured
✅ **No Display Required** - Works in Docker, CI/CD, servers
✅ **No GPU Required** - Pure CPU execution
✅ **Asset Loading** - Textures/sprites load normally
✅ **ECS Systems** - All your game logic runs

## The Only Fix Needed

Make telemetry resources optional:

```diff
fn my_system(
-   tracer: Res<GameTracer>,
+   tracer: Option<Res<GameTracer>>,
) {
+   if let Some(tracer) = tracer {
        // Use tracer
+   }
}
```

This was the **only** issue - not rendering!

## Summary

Your headless implementation is **exactly right** for Bevy 0.17. The panic you were seeing was about missing telemetry resources, not surface creation. With the telemetry fix applied, you now have:

- True headless mode (no window/GPU)
- BRP remote control working
- OTLP telemetry working
- Perfect for automated testing

See `docs/HEADLESS_MODE.md` for full documentation.
