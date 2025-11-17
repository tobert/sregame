# Headless Mode Quick Start

## TL;DR

```bash
# Run headless for 5 seconds
cargo run --release -- --headless --seconds 5

# Headless + Bevy Remote Protocol
cargo run --release -- --headless --remote --seconds 10

# Headless + OpenTelemetry
cargo run --release -- --headless --otlp-endpoint 127.0.0.1:4317 --seconds 10
```

## What It Does

- ✅ Runs game logic without creating windows
- ✅ No display server (X11/Wayland) required
- ✅ Works in CI/CD environments
- ✅ Compatible with BRP and OpenTelemetry
- ✅ Fixed 60 FPS event loop
- ✅ All game systems run normally

## When To Use

| Use Case | Command |
|----------|---------|
| CI/CD automated tests | `--headless --seconds 30` |
| BRP remote testing | `--headless --remote --seconds 60` |
| Performance profiling | `--headless --frames 3600` |
| Integration tests | `--headless --seconds 10` |
| Docker containers | `--headless --remote` |

## How It Works

```
┌─────────────────┐
│  Normal Mode    │
├─────────────────┤
│ Winit Plugin    │ ← Requires display server
│ Window Creation │ ← Can hang in headless environments
│ GPU Rendering   │
│ Game Logic      │
└─────────────────┘

┌─────────────────┐
│ Headless Mode   │
├─────────────────┤
│ ScheduleRunner  │ ← Timer-based event loop (no display needed)
│ Game Logic      │ ← Everything works normally
│ (No Window)     │ ← Perfect for CI/CD
└─────────────────┘
```

## Technical Details

| Component | Normal | Headless |
|-----------|--------|----------|
| WindowPlugin | Creates window | `primary_window: None` |
| WinitPlugin | Enabled | **Disabled** (prevents hang) |
| Event Loop | Winit window loop | ScheduleRunnerPlugin (60 FPS) |
| Exit Condition | OnAllClosed | DontExit (manual control) |
| Display Server | Required | **Not required** |
| GPU | Used for rendering | Initializes but no output |

## Exit Strategies

The game needs to know when to exit in headless mode:

```bash
# Time-based (recommended for most tests)
--seconds 10

# Frame-based (deterministic, good for profiling)
--frames 600  # 10 seconds at 60fps

# Manual (via BRP or signals)
# Send AppExit event via BRP, or use Ctrl+C
```

## Verification

Check it's actually running headless:

```bash
# Should see this in output:
# "🔧 Running in headless mode (no window, no display server required)"

cargo run --release -- --headless --seconds 3 2>&1 | grep headless
```

## Troubleshooting

**Game exits immediately:**
- Ensure you set `--seconds N` or `--frames N`
- Headless mode doesn't auto-exit without these flags

**Systems not running:**
- `Update` schedule runs at 60 FPS via ScheduleRunnerPlugin
- Check logs for system execution

**Assets not loading:**
- Assets load normally in headless mode
- Ensure `assets/` directory is in the correct location
- Use `--release` and run from project root, or assets will be at `target/release/assets/`

## See Also

- Full documentation: `docs/headless-mode.md`
- Test script: `test_headless.sh`
- Bevy examples: `/home/atobey/src/bevy/examples/app/headless*.rs`
