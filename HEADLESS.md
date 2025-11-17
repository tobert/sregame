# Headless Mode Support

The Endgame of SRE supports true headless operation for CI/CD, automated testing, and remote control via Bevy Remote Protocol.

## Quick Start

```bash
# Run headless for 5 seconds
cargo run --release -- --headless --seconds 5

# Run headless with Bevy Remote Protocol
cargo run --release -- --headless --remote --seconds 30

# Run automated test suite
./test_headless.sh
```

## Features

✅ **No display server required** - Works in Docker, CI/CD, headless servers
✅ **Full game logic execution** - All systems run normally at 60 FPS
✅ **Bevy Remote Protocol compatible** - Control game state remotely
✅ **OpenTelemetry integration** - Export logs and metrics to OTLP collectors
✅ **Deterministic exit** - Time-based (`--seconds`) or frame-based (`--frames`)
✅ **CI/CD ready** - No GPU or window manager needed

## Documentation

- **Quick Start**: [`docs/headless-quickstart.md`](/home/atobey/src/sregame/docs/headless-quickstart.md) - Commands and use cases
- **Full Guide**: [`docs/headless-mode.md`](/home/atobey/src/sregame/docs/headless-mode.md) - Technical details and implementation
- **Test Suite**: [`test_headless.sh`](/home/atobey/src/sregame/test_headless.sh) - Automated verification

## Architecture

Headless mode uses Bevy 0.17's `ScheduleRunnerPlugin` to replace the default Winit window manager:

```rust
// Key components
.set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit })
.disable::<WinitPlugin>()  // Prevents display server connection
.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)))
```

This avoids the hang you experienced with Wayland compositors by:
1. **Not creating windows** - `primary_window: None`
2. **Disabling Winit** - Prevents display server connection attempts
3. **Custom event loop** - Timer-based instead of window-event-based
4. **Manual exit control** - Your `--seconds`/`--frames` flags control shutdown

## Use Cases

| Scenario | Command | Why |
|----------|---------|-----|
| GitHub Actions CI | `--headless --seconds 30` | No display server |
| Docker containers | `--headless --remote` | Remote control via BRP |
| Integration tests | `--headless --frames 600` | Deterministic timing |
| Performance profiling | `--headless --seconds 60` | Pure logic, no rendering overhead |
| OTLP log collection | `--headless --otlp-endpoint 127.0.0.1:4317 --seconds 30` | Automated telemetry testing |

## Testing

```bash
# Quick verification (3 seconds)
cargo run --release -- --headless --seconds 3

# Full test suite (all modes)
./test_headless.sh

# CI/CD example
cargo build --release
timeout 60 ./target/release/sregame --headless --seconds 30
```

## CI/CD Integration

### GitHub Actions

```yaml
- name: Headless test
  run: cargo run --release -- --headless --seconds 10
```

### Docker

```dockerfile
CMD ["./target/release/sregame", "--headless", "--remote"]
```

## Technical Notes

- **Bevy Version**: 0.17 (headless support varies by version)
- **FPS**: Fixed 60 FPS via ScheduleRunnerPlugin
- **GPU**: Initializes Vulkan/WGPU but renders to null target
- **Assets**: Load normally (ensure `assets/` is accessible)
- **Systems**: All schedules run (`Startup`, `Update`, `PostUpdate`, etc.)

## References

- Bevy 0.17 headless example: `/home/atobey/src/bevy/examples/app/headless.rs`
- Implementation: [`src/main.rs`](/home/atobey/src/sregame/src/main.rs) lines 115-146
- Original issue: Wayland compositor hang during window creation

## License

Same as project (see LICENSE)

---

**Questions?** See [`docs/headless-mode.md`](/home/atobey/src/sregame/docs/headless-mode.md) for complete technical documentation.
