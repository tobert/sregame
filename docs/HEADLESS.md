# Running sregame Without a Display

There are two headless modes, and they answer different questions. Every
command below has been verified working on this machine (2026-07-12).

| | `cargo run -- --headless` | `scripts/run-headless.sh` |
|---|---|---|
| Window / GPU | none — rendering disabled | full Vulkan pipeline on a gamescope virtual display |
| Frames exist? | no | yes — real, screenshot-able |
| Good for | logic/telemetry smoke tests, CI, "does it spawn without panicking?" | visual verification, BRP screenshots, "does it *look* right?" |
| Requirements | nothing | `gamescope` (Arch: `sudo pacman -S gamescope`) |
| Code path | `WindowPlugin { primary_window: None }` + `ScheduleRunnerPlugin` at 60fps | identical to a normal windowed run |

Use both: `--headless` when you only need logs, gamescope when a claim is
visual. History lesson: the vertically-mirrored-map bug shipped precisely
because nothing in the loop could *see* the game.

## Logic smoke test (no GPU)

```bash
cargo run -- --headless --seconds 6 2>&1 | grep -E "NPC spawned|panic"
```

`--seconds N` / `--frames N` make the run self-terminating — always pass one
when launching from automation so orphaned processes can't accumulate.

## Visual run (gamescope)

```bash
# Everything before -- goes to cargo, after it to the game.
./scripts/run-headless.sh -- --remote --remote-port 15799 --seconds 60
```

`--remote` serves the Bevy Remote Protocol over HTTP with the
`bevy_brp_extras` methods (`brp_extras/screenshot`, `send_keys`, `shutdown`,
`set_window_title`). Pick a `--remote-port`: the default 15702 is often
occupied by other Bevy apps on this machine (`ss -tlnp | grep 15702` to
check).

### Screenshot

```bash
curl -s -X POST http://127.0.0.1:15799/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/screenshot",
       "params":{"path":"/tmp/sregame.png"}}'
```

The write is async on a background thread — allow a beat before reading the
file. A 1920x1080 frame lands wherever `path` points (relative paths resolve
against the game's working directory).

### Clean shutdown

```bash
curl -s -X POST http://127.0.0.1:15799/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"brp_extras/shutdown"}'
```

## Telemetry in either mode

```bash
cargo run -- --headless --otlp-endpoint 127.0.0.1:4317 --seconds 30
```

Or set `OTEL_EXPORTER_OTLP_ENDPOINT`. With telemetry on, the player's
session span carries live `player.x`, `player.y`, and `game.scene`
attributes (see `src/semantic_state.rs`), so the OTLP stream alone can
answer "where is the player?".

For the agent-facing workflow that ties all of this together, see
`docs/agents/AUTONOMY_GUIDE.md`.
