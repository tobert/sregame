# Headless Development Test Scripts

Collection of scripts for testing headless game execution, Bevy Remote Protocol, and telemetry integration.

## Scripts

### `test-headless-cage.sh`
Tests basic headless execution using the Cage Wayland compositor.

```bash
# Run with defaults (3 seconds)
./scripts/test-headless-cage.sh

# Run for 10 seconds
./scripts/test-headless-cage.sh --seconds 10

# Run with custom timeout
HEADLESS_TIMEOUT=30 ./scripts/test-headless-cage.sh --seconds 5
```

### `test-remote-protocol.sh`
Tests Bevy Remote Protocol (BRP) connectivity.

```bash
# Run with defaults
./scripts/test-remote-protocol.sh

# Custom test duration
TEST_DURATION=10 ./scripts/test-remote-protocol.sh

# Custom BRP port
BRP_PORT=8080 ./scripts/test-remote-protocol.sh
```

**Requirements:**
- `curl` for HTTP testing
- `jq` for pretty JSON output (optional)

### `test-telemetry.sh`
Tests OpenTelemetry OTLP telemetry integration.

```bash
# Run with defaults (uses MCP OTLP endpoint)
./scripts/test-telemetry.sh

# Custom OTLP endpoint
OTEL_EXPORTER_OTLP_ENDPOINT=127.0.0.1:4317 ./scripts/test-telemetry.sh

# Custom test duration
TEST_DURATION=10 ./scripts/test-telemetry.sh
```

**Requirements:**
- OTLP-MCP running (via Claude Code or standalone)

### `test-full-stack.sh`
Combines all features: Headless + BRP + Telemetry.

```bash
# Run full integration test
./scripts/test-full-stack.sh

# Custom configuration
OTLP_ENDPOINT=127.0.0.1:4317 \
BRP_PORT=15702 \
TEST_DURATION=10 \
HEADLESS_TIMEOUT=30 \
  ./scripts/test-full-stack.sh
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HEADLESS_TIMEOUT` | 15 | Timeout in seconds for headless tests |
| `TEST_DURATION` | 5 | How long the game should run |
| `BRP_PORT` | 15702 | Bevy Remote Protocol port |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | 127.0.0.1:42701 | OTLP endpoint for telemetry |

## Troubleshooting

### Game hangs during headless execution
The game currently hangs after Vulkan initialization when run with `cage`. This is a known issue being investigated. Symptoms:
- Cage initializes successfully (EGL, OpenGL, GPU detection)
- Game compiles and starts
- Hangs before entering the main game loop

**Workarounds being tested:**
1. Using `weston-headless` instead of `cage`
2. Setting `WL_OUTPUT` environment variables
3. Using `xvfb-run` with X11 backend

### BRP not accessible
If `test-remote-protocol.sh` fails to connect:
1. Ensure `--remote` flag is being used
2. Check the port isn't already in use: `lsof -i :15702`
3. Look for BRP initialization logs in game output

### Telemetry not flowing
If telemetry test shows no data:
1. Verify OTLP-MCP is running (check Claude Code MCPs)
2. Confirm the endpoint is correct
3. Check firewall isn't blocking the port
4. Look for telemetry initialization logs in game output

## Logs

Test logs are saved to `/tmp/sregame-*-test.log` for debugging.

## See Also

- [Bevy Remote Protocol Docs](https://docs.rs/bevy_remote/)
- [OpenTelemetry OTLP Spec](https://opentelemetry.io/docs/specs/otlp/)
- `docs/agents/CONTEXT.md` - Current headless implementation status
