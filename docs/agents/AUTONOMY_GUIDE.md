# Autonomy Guide — Running and Verifying sregame as an Agent

How an AI agent builds, runs, drives, and — most importantly — *sees* this
game. Rewritten 2026-07-12 after the workflow was proven end-to-end; every
command here has actually been run.

The one rule: **match the evidence to the claim.** Logs prove the game
didn't crash. OTLP proves behavior. Only a rendered frame proves the game
looks right — the vertically-mirrored-map bug survived multiple sessions of
log-based and static analysis because nobody could look at a frame.

## Launch modes

```bash
# Logic / telemetry only (no GPU, works anywhere):
cargo run -- --headless --seconds 30

# Real rendered frames on a virtual display (needs gamescope):
./scripts/run-headless.sh -- --remote --remote-port 15799 --seconds 120
```

Rules of engagement:

- **Always pass `--seconds N`** from automation. It is your watchdog; an
  agent that forgets its background processes should not leave a game
  running forever.
- **Always pick an explicit `--remote-port`.** 15702 (the BRP default) is
  routinely occupied by other Bevy apps on this machine. Check with
  `ss -tlnp | grep <port>`.
- Launch as a background task and poll the BRP port for readiness rather
  than sleeping a fixed guess:

  ```bash
  for i in $(seq 1 30); do
    curl -s -m 2 -X POST http://127.0.0.1:15799/ \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}' \
      | grep -q jsonrpc && break
    sleep 1
  done
  ```

## Seeing the game (the important part)

With `--remote` up, capture a frame and *look at it* (multimodal models:
read the PNG directly):

```bash
curl -s -X POST http://127.0.0.1:15799/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/screenshot",
       "params":{"path":"/tmp/sregame-frame.png"}}'
# File write is async — wait ~1s before reading.
```

Screenshot before and after a change when verifying anything visual:
sprite selection, map orientation, z-ordering, dialogue layout.

## Driving the game

`brp_extras/send_keys` injects input, which is enough to walk Amy around
and talk to NPCs (WASD/arrows to move, E to interact, Space/Enter to
advance dialogue, Escape to close it):

```bash
curl -s -X POST http://127.0.0.1:15799/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/send_keys",
       "params":{"keys":["KeyW"],"duration_ms":500}}'
```

Standard BRP methods (`world.query`, `world.get_components`, mutation,
etc.) are also live; game components are registered for reflection
(`sregame::player::Player`, `Velocity`, `Facing`, `sregame::npc::Npc`, …).
If a component comes back "not registered", it needs `#[derive(Reflect)]` +
`app.register_type::<T>()` — add it rather than working around it.

If a `bevy_brp_mcp` MCP server is configured (see `bevy_mcp_config.json`
and `.gemini/settings.json`), the same operations are available as native
MCP tools; raw `curl` JSON-RPC is the always-works fallback.

## Watching behavior (OTLP)

Get the endpoint from the otlp-mcp server (its `get_otlp_endpoint` tool),
then launch the game with it — substitute the actual value; MCP tools are
not shell commands:

```bash
cargo run -- --headless --otlp-endpoint 127.0.0.1:4317 --seconds 60
```

What flows: logs, spans (`game_session` → `npc.interaction` →
`dialogue.session`), metrics, and — via `src/semantic_state.rs` — live
`player.x` / `player.y` / `game.scene` attributes on the session span, with
a `player.state_update` span event whenever the player actually moves or
changes scene. The snapshot workflow (snapshot → act → snapshot → diff) is
the cleanest way to tie telemetry to an action you just performed.

## Cleanup

```bash
curl -s -X POST http://127.0.0.1:15799/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":9,"method":"brp_extras/shutdown"}'
```

The `--seconds` watchdog is the backstop if the shutdown call is missed.
Delete any OTLP snapshots you created.

## Failure modes seen in practice

- **BRP connection refused**: game still compiling (first `cargo run` in a
  cold tree takes minutes) or wrong port. Poll; don't guess with sleeps.
- **Screenshot file missing**: the write is async — wait and re-check
  before concluding it failed.
- **Port 15702 in use**: that's not your game. Use `--remote-port`.
- **Static analysis said the art is broken**: render a frame before filing
  the bug. Sparse-looking data is sometimes legitimate content.
