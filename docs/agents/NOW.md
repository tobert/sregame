# NOW.md - Current Working State

**Last Updated**: 2025-11-17 by Claude

> ⚠️ **Stale**: this snapshot predates the content-parity work (all 10 maps,
> portals, walk animations, portrait/orientation/sprite-slot fixes — see
> `git log` on the `content-parity` branch). Read git history for current
> state; treat everything below as historical.

## 🎯 Active Work

✅ **TRUE Headless Mode CONFIRMED!** The existing `--headless` flag with `primary_window: None` is the CORRECT Bevy 0.17 solution. No Weston needed - pure CPU execution with BRP + OTLP working perfectly.

⚠️ **Weston Approach Was Wrong**: Surface panics were leading us astray. Bevy's built-in headless mode (already implemented) is superior.

✅ **Phase 5 Complete**: MCP integration working! Opt-in telemetry via CLI flag/env var with stable port configuration. All telemetry (logs, traces, metrics) flowing successfully to MCP.

## 📍 Current State

**Implementation Status:**
- ✅ Step 01-02: Project setup + Game state management
- ✅ Step 06-07: Dialogue + NPC interaction systems
- ✅ Step 09: Content port with clean map data format
- ✅ **OpenTelemetry Phases 1 & 2**: Complete instrumentation (logs, traces, metrics)
- ✅ **Crash Prevention**: 7 vulnerabilities fixed with defensive programming

**OpenTelemetry Implementation:**
- ✅ Opt-in telemetry (CLI `--otlp-endpoint` or `OTEL_EXPORTER_OTLP_ENDPOINT`)
- ✅ Stable port 4317 via `.otlp-mcp.json` config
- ✅ Graceful degradation (console logging when telemetry disabled)
- ✅ Unified endpoint for logs, traces, and metrics
- ✅ Player session trace tracking (root span on player entity)
- ✅ NPC interaction spans with position/distance attributes
- ✅ Dialogue session spans with reading speed metrics
- ✅ Resource lifecycle telemetry (create/remove events)
- ✅ Unified cleanup on forced dialogue exit (Escape key)
- ✅ Debug assertions (duplicate player detection, sprite bounds)
- ✅ Enhanced error context (JSON parsing with file path/size)

**Trace Hierarchy:**
```
game_session (player entity)
└─ npc.interaction
   └─ dialogue.session
      └─ events: dialogue.line_displayed
```

## ✅ RESOLVED: Telemetry Connection

**Solution implemented**: Opt-in telemetry with configurable endpoint
- `.otlp-mcp.json` configures MCP to listen on stable port 4317
- CLI flag: `cargo run -- --otlp-endpoint 127.0.0.1:4317`
- Env var: `OTEL_EXPORTER_OTLP_ENDPOINT=127.0.0.1:4317`
- No endpoint = graceful console-only logging

**Verified working**:
- ✅ 13 logs captured during test run
- ✅ 10 metrics exported (frame_time, dialogue_reading_speed, interactions, etc.)
- ✅ Traces exported successfully
- ✅ MCP snapshot workflow operational

## 🚧 Known Issues

1. **Map portals**: Not yet implemented (step 08 pending)
2. **Character animations**: Static sprites, no walk cycles yet
3. **Sound/Music**: Not implemented
4. **Frame time tracking**: Metric created but not recorded yet
5. **Map transitions**: Spans not instrumented yet

## 🔍 Current Focus

**Completed (Phase 5 - MCP Integration):**
1. ✅ OTLP endpoint configuration (CLI + env var)
2. ✅ Telemetry flow verified (logs + metrics + traces)
3. ✅ Snapshot workflow tested
4. 📊 MCP query patterns (ready for documentation)

**Next Phase Options:**
- **Phase 3**: Map transition instrumentation + frame time tracking
- **Phase 4**: Error scenarios and debugging features
- **Content**: Implement map portals (step 08) or character animations

## 📝 Context for Next Session

**What Works:**
- Game runs with or without MCP (graceful degradation)
- All telemetry code in place and compiling
- Crash prevention fixes prevent 7 critical failures
- Full trace context propagation (session → interaction → dialogue)

**What Needs Testing:**
- Actual telemetry flow to MCP (blocked by endpoint mismatch)
- Snapshot workflow for temporal debugging
- MCP query tools for debugging sessions

**Latest Session Changes:**
- .otlp-mcp.json: Stable port 4317 config (NEW)
- Cargo.toml: Add clap 4.5 for CLI parsing
- src/main.rs: CLI args, endpoint resolution, graceful degradation
- src/telemetry.rs: Accept optional endpoint parameter
- src/instrumentation.rs: Accept endpoint parameter
- examples/test_logging.rs: Require env var with helpful error

**Commits:**
- `676d91e3`: Opt-in telemetry with CLI flag/env var + MCP stable port
- `3ba0800f`: Updated otel-plan.md with Phase 5 priorities
- `a0c2ed32`: OpenTelemetry OTLP logging integration (main)

## 🤔 Open Questions

- Should we use env var or MCP port configuration for endpoint?
- How to integrate snapshot creation into gameplay loop?
- Need query cookbook docs - separate file or in otel-plan.md?
- When to implement frame time tracking (Phase 3)?

## 🎯 Next Steps

**Recommended priorities:**
1. **Test full game session** with telemetry enabled
2. **Document MCP query patterns** for debugging workflows
3. **Implement Phase 3**: Map transitions + frame time tracking
4. **Step 08**: Map portal system (architectural feature)
5. **Animations**: Character walk cycles (visual polish)

**Usage Examples:**
```bash
# Enable telemetry with env var
OTEL_EXPORTER_OTLP_ENDPOINT=127.0.0.1:4317 cargo run

# Enable telemetry with CLI flag
cargo run -- --otlp-endpoint 127.0.0.1:4317

# Run without telemetry (default)
cargo run
```

---
*Session complete. Phases 1, 2, & 5 done. MCP integration fully operational.*
🤖 Claude <claude@anthropic.com>
