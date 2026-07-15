---
name: verify
description: Runtime verification recipes for sregame — GPU-rendered native frames via gamescope+BRP, and web canvas screenshots via headless chromium. Use when a change needs visual/behavioral evidence, not just cargo check.
---

# Verifying sregame changes at runtime

Two proven loops. Pick by surface: native window behavior → gamescope+BRP;
web/canvas behavior (fit_canvas_to_parent, small embeds) → chromium.

## Native: gamescope headless + BRP

```bash
# terminal 1 (background): real GPU frames on a virtual display, 1920x1080
./scripts/run-headless.sh -- --remote --remote-port 15799 --seconds 240
```

Port 15702 is squatted by kaijutsu-app — always pass `--remote-port`.
Poll until BRP answers, then drive and capture:

```bash
# screenshot (file write is async — sleep ~2s before reading the PNG)
curl -s -X POST http://127.0.0.1:15799/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/screenshot","params":{"path":"/abs/path.png"}}'

# hold a key (walk the player); returns after duration
curl -s -X POST http://127.0.0.1:15799/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"brp_extras/send_keys","params":{"keys":["ArrowLeft"],"duration_ms":6000}}'
```

Also available: `brp_extras/shutdown`. Player world coords print every 1s in
the game log as a `[VIEWPORT]` ASCII map (src/viewport.rs) — use it to confirm
where the player actually is when sprites are occluded (roofs hide the player).

## Web: bevy CLI + headless chromium

```bash
bevy run web --port 4000        # ~3min cold compile; poll GET / for 200
chromium --headless=new --enable-unsafe-swiftshader --virtual-time-budget=30000 \
  --window-size=640,360 --screenshot=/abs/path.png http://127.0.0.1:4000
```

`--window-size` sets the canvas size (fit_canvas_to_parent), so this is how to
test blog-embed-sized views. WebGL runs on SwiftShader; without
`--enable-unsafe-swiftshader` the screenshot may be blank. One-shot
`--screenshot` mode can't send key input — driving gameplay on web needs CDP;
use the native BRP loop for anything requiring movement/dialogue.

## Gotchas

- `--headless` (the game flag) renders no frames — logic/log smoke tests only.
- Reaching late-game scenes (inn retro room) requires real progression;
  budget send_keys sequences or verify the mechanism on the town map.
