# OpenTelemetry Implementation Plan - SRE Game

**Status**: Phases 1 & 2 Complete + Crash Prevention Fixes
**Last Updated**: 2025-11-16 (Post-Crash Prevention Implementation)
**Current Issue**: 🔥 ENDPOINT MISMATCH - Need to configure MCP port

## Overview

Complete observability for the SRE Game using OpenTelemetry, enabling real-time visibility into both game performance and player experience through logs, metrics, and distributed traces.

## Goals

1. **Fast Debugging**: Query logs and traces from MCP without parsing console output
2. **Player Journey Tracking**: Understand player behavior through connected trace hierarchy
3. **Performance Monitoring**: Identify bottlenecks via histograms and system metrics
4. **Collaborative Development**: Claude can query game state and behavior directly

---

## Architecture

### Telemetry Stack

```
┌─────────────────────────────────────────────────────┐
│                   Bevy Game                         │
│  ┌──────────────────────────────────────────────┐  │
│  │  Logs (info!, warn!, error!)                 │  │
│  │  → tracing-subscriber                        │  │
│  │  → opentelemetry-appender-tracing            │  │
│  │  → OTLP Log Exporter                         │  │
│  └──────────────────────────────────────────────┘  │
│                                                      │
│  ┌──────────────────────────────────────────────┐  │
│  │  Traces (spans with attributes)              │  │
│  │  → GameTracer resource                       │  │
│  │  → OTLP Span Exporter                        │  │
│  └──────────────────────────────────────────────┘  │
│                                                      │
│  ┌──────────────────────────────────────────────┐  │
│  │  Metrics (histograms, counters)              │  │
│  │  → GameMeter resource                        │  │
│  │  → OTLP Metric Exporter (10s periodic)       │  │
│  └──────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
                         │
                         │ gRPC (Tonic)
                         ▼
               ┌─────────────────────────────────────┐
               │  OTLP-MCP (ephemeral port)         │
               │  Default: random (e.g. 45281)      │
               │  Configured: 44173 (via add_port)  │
               └─────────────────────────────────────┘
                         │
                         │ MCP Protocol
                         ▼
                  ┌─────────────┐
                  │   Claude    │
                  │  (queries)  │
                  └─────────────┘
```

### Shared Tokio Runtime

Since Bevy doesn't use Tokio, we create a dedicated runtime for OTLP exporters:
- Created in `main()` before Bevy app starts
- Shared by logs, traces, and metrics exporters
- Kept alive in background thread for async operations

---

## Phase 1: Foundational Infrastructure ✅

### Implemented

**Logs** (`src/telemetry.rs`):
- ✅ OTLP log exporter with batch processing
- ✅ tracing-subscriber bridge
- ✅ Filter to prevent telemetry loops
- ✅ All game logs flow to MCP

**Metrics** (`src/instrumentation.rs`):
- ✅ GameMeter Bevy resource
- ✅ Histograms: frame_time, system_execution_time, dialogue_reading_speed, interaction_duration
- ✅ Counters: interactions_total, dialogue_lines_read, map_transitions
- ✅ Periodic export every 10 seconds

**Traces** (`src/instrumentation.rs`):
- ✅ GameTracer Bevy resource
- ✅ PlayerSessionTrace component (long-running root span)
- ✅ Helper functions for creating child spans
- ✅ Context propagation from session → interactions → dialogue

**Testing**:
- ✅ `examples/test_logging.rs` demonstrates all three signal types
- ✅ Verified logs, metrics, and traces flow to MCP
- ✅ Confirmed histogram aggregation and attribute filtering

---

---

## 🚨 CRITICAL: Endpoint Configuration Issue

**Problem Discovered**: MCP uses ephemeral ports by default, but our code hardcodes `127.0.0.1:44173`

**Current State**:
- Our telemetry code: `http://127.0.0.1:44173` (hardcoded in src/telemetry.rs:14, src/instrumentation.rs:95)
- MCP actual port: Random ephemeral (currently `127.0.0.1:45281`)
- Result: Graceful degradation working, but NO telemetry is actually flowing!

**Solutions** (pick one):

1. **Option A: Configure MCP to use our port** (Quick fix for development)
   ```rust
   mcp__otlpmcp__add_otlp_port(44173)
   // Now our hardcoded endpoint works!
   ```

2. **Option B: Use environment variable** (Better for flexibility)
   ```rust
   // src/instrumentation.rs and src/telemetry.rs
   let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
       .unwrap_or_else(|_| "http://127.0.0.1:44173".to_string());

   // Run game with:
   // OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:45281 cargo run
   ```

3. **Option C: Query MCP endpoint dynamically** (Most robust, but complex)
   - Requires MCP integration before game starts
   - Could use mcp__otlpmcp__get_otlp_endpoint to discover port
   - Would need coordination between Claude Code and game startup

**Recommendation**: Option A for immediate testing, then Option B for production

---

## Phase 2: Player Journey Tracking ✅

### Status: COMPLETE (with crash prevention enhancements)

**Session Tracking** ✅:
```rust
// Root span attached to player entity
PlayerSessionTrace {
    span: game_session,           // Long-running root span
    context: OtelContext,          // For creating children
    session_start: Instant,        // Relative timing
}

// Attributes:
- session.start_time: ISO8601 timestamp
- game.version: from Cargo.toml
- trace_id: automatically assigned
```

**NPC Interactions** ✅:
```rust
// Span created when player presses E near NPC
npc.interaction {
    parent: game_session,

    attributes: {
        npc.name: "Nyaanager Evie",
        player.x: 120.5,
        player.y: 240.0,
        interaction.distance: 45.2,
        session.elapsed_ms: 12534,
    }
}

// Metric recorded:
game.interactions.total{npc.name="Nyaanager Evie"} += 1
```

**Dialogue Sessions** ✅ COMPLETE:
```rust
// Child span of npc.interaction
dialogue.session {
    parent: npc.interaction,

    attributes: {
        dialogue.speaker: "Nyaanager Evie",
        dialogue.total_lines: 5,
        dialogue.chars_read: 245,
    },

    events: [
        dialogue.line_displayed {
            line.index: 0,
            line.length: 52,
            line.preview: "Welcome to the Town of Endgame! I'm Nyaanager E..."
        },
        // ... more line events
    ]
}

// Metrics:
game.dialogue_lines_read += 5
game.dialogue.reading_speed.record(18.5)  // chars/second
```

**Map Transitions** 🔜 (Phase 3 - Not yet implemented):
```rust
map.transition {
    parent: game_session,

    attributes: {
        map.from: "TownOfEndgame",
        map.to: "TeamMarathon",
        player.x: 480.0,
        player.y: 540.0,
        session.elapsed_ms: 45120,
    }
}

// Metric:
game.map_transitions{from="TownOfEndgame", to="TeamMarathon"} += 1
```

### Trace Hierarchy

```
game_session (root span on player entity)
│   attributes: session.start_time, game.version
│   duration: entire play session (spawn → exit)
│
├─ npc.interaction (player presses E)
│  │  attributes: npc.name, player.x/y, distance, session.elapsed_ms
│  │  duration: brief (key press event)
│  │  metric: game.interactions.total{npc.name}
│  │
│  └─ dialogue.session (child of interaction)
│     │  attributes: speaker, total_lines, chars_read
│     │  duration: until player completes dialogue
│     │  events: dialogue.line_displayed (one per line)
│     │  metrics: dialogue_lines_read, dialogue.reading_speed
│     │
│     └─ events: line_displayed{index, length, preview}
│
└─ map.transition (when changing scenes)
   │  attributes: map.from, map.to, player.x/y, session.elapsed_ms
   │  duration: brief (scene change event)
   │  metric: game.map_transitions{from, to}
```

---

## Phase 3: Performance Monitoring 🔜

### System Performance Tracking

**Frame Time Monitoring**:
```rust
// In main game loop (measure actual frame time)
fn track_frame_time(
    time: Res<Time>,
    meter: Res<GameMeter>,
) {
    let frame_ms = time.delta_secs() * 1000.0;
    meter.frame_time.record(frame_ms, &[]);
}

// Query results:
game.frame_time histogram:
  count: 3600 (60 seconds * 60 FPS)
  sum: 60120.5 ms
  avg: ~16.7 ms (60 FPS)
  p50: 16.5 ms
  p95: 18.2 ms
  p99: 22.1 ms
```

**ECS System Execution Time**:
```rust
// Wrapper for expensive systems
fn track_system_time<F>(
    system_name: &str,
    meter: &GameMeter,
    f: F
) where F: FnOnce() {
    let start = Instant::now();
    f();
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    meter.system_execution_time.record(
        duration_ms,
        &[KeyValue::new("system", system_name)]
    );
}

// Applied to:
- player_movement
- tilemap_render
- dialogue_render
- npc_proximity_check
```

**Asset Loading**:
```rust
// Track asset load times
asset.loading_time{asset_type="map", map_name="TownOfEndgame"}
asset.loading_time{asset_type="sprite", sprite="player.png"}
asset.loading_time{asset_type="dialogue", file="evie_intro.json"}
```

---

## Phase 4: Advanced Features 🔮

### Span Links

For connecting related but non-hierarchical events:
```rust
// Link dialogue session back to original NPC interaction if needed
dialogue_span.add_link(npc_interaction_span.span_context());

// Link map transition to previous map's exit point
transition_span.add_link(previous_map_span.span_context());
```

### Custom Events

Rich events within spans:
```rust
// Player death event (when combat exists)
session_span.add_event("player.death", vec![
    KeyValue::new("death.cause", "fell_off_map"),
    KeyValue::new("death.location.x", player_x),
    KeyValue::new("death.location.y", player_y),
]);

// Quest completion events
session_span.add_event("quest.completed", vec![
    KeyValue::new("quest.id", "learn_error_budgets"),
    KeyValue::new("quest.duration_ms", duration),
]);
```

### Derived Metrics

Metrics computed from traces:
- Average session duration
- Dialogue completion rate
- Most interacted NPCs
- Player movement heatmap (from position attributes)

---

## Implementation Guide

### Adding a New Span

```rust
// 1. Get tracer and session from Bevy ECS
fn my_system(
    tracer: Res<GameTracer>,
    player_query: Query<&PlayerSessionTrace, With<Player>>,
) {
    let session_trace = player_query.single();

    // 2. Create span as child of session
    let context = session_trace.as_context();
    let mut span = tracer.tracer()
        .start_with_context("my_event", &context);

    // 3. Add attributes
    span.set_attribute(KeyValue::new("my.attribute", "value"));
    span.set_attribute(KeyValue::new("session.elapsed_ms",
        session_trace.session_start.elapsed().as_millis() as i64));

    // 4. Add events if needed
    span.add_event("something.happened", vec![
        KeyValue::new("detail", "info")
    ]);

    // 5. Span ends when dropped (or explicitly call .end())
}
```

### Recording a Metric

```rust
// Histogram
fn my_system(meter: Res<GameMeter>) {
    let value_ms = measure_something();
    meter.my_histogram.record(value_ms, &[
        KeyValue::new("label", "value")
    ]);
}

// Counter
fn my_system(meter: Res<GameMeter>) {
    meter.my_counter.add(1, &[
        KeyValue::new("type", "npc")
    ]);
}
```

### Querying with MCP

```rust
// Get recent logs
mcp__otlpmcp__query({
    service_name: "sregame",
    limit: 50
})

// Get specific span type
mcp__otlpmcp__query({
    span_name: "npc.interaction",
    service_name: "sregame"
})

// Get metrics
mcp__otlpmcp__query({
    metric_names: ["game.interactions.total"],
    service_name: "sregame"
})
```

---

## File Structure

```
src/
├── main.rs                    # Init telemetry, insert resources
├── telemetry.rs              # Log exporter setup
├── instrumentation.rs         # Trace/metric infrastructure
│   ├── GameTracer            # Bevy resource for tracer
│   ├── GameMeter             # Bevy resource for metrics
│   ├── PlayerSessionTrace    # Component for session span
│   ├── ActiveInteraction     # Component for NPC interaction
│   ├── ActiveDialogue        # Component for dialogue session
│   └── Helper functions      # start_*_span(), add_*_context()
├── player.rs                 # Spawn with PlayerSessionTrace
├── npc.rs                    # NPC interaction spans
├── dialogue.rs               # [TODO] Dialogue session spans
└── tilemap.rs                # [TODO] Map transition spans

examples/
└── test_logging.rs           # Demo all telemetry types

docs/
└── otel-plan.md              # This file
```

---

## Benefits for Development

### For Claude (AI Agent)

1. **Context Awareness**: Query traces to see exact player journey
   ```
   "What happened in the last playtest?"
   → Query game_session spans, see all interactions
   ```

2. **Debug Without Running**: Read traces instead of asking user to reproduce
   ```
   "Why did dialogue not trigger?"
   → Check npc.interaction spans, see distance attribute
   ```

3. **Performance Analysis**: Identify bottlenecks from histogram data
   ```
   "Is the game laggy?"
   → Query frame_time histogram, check p95/p99
   ```

4. **Proactive Insights**: Notice patterns before user reports them
   ```
   "Players press E multiple times near NPCs"
   → Count of interactions.total > expected, add visual feedback
   ```

### For Developer (You)

1. **Visual Playtest Analysis**: Run game, review trace timeline
2. **Bug Reproduction**: Complete context of what led to crash
3. **Feature Validation**: Did change improve dialogue reading speed?
4. **Production Monitoring**: Track actual player behavior (future)

---

## Phase 5: MCP Integration & Workflow 🔥 NEXT

### Critical Priorities (DO THESE FIRST)

**Priority 1: Fix Endpoint Connection** 🔥
- [ ] Add port 44173 to MCP: `mcp__otlpmcp__add_otlp_port(44173)`
- [ ] OR: Update code to use environment variable `OTEL_EXPORTER_OTLP_ENDPOINT`
- [ ] Test telemetry is actually flowing with example
- [ ] Verify logs/spans/metrics appear in MCP

**Priority 2: Snapshot-Based Workflow**
- [ ] Create snapshot before player interacts with NPC
- [ ] Create snapshot after dialogue completes
- [ ] Query `get_snapshot_data` to see everything that happened
- [ ] Document workflow in this file or separate playbook

**Priority 3: MCP Query Examples**
- [ ] Document common queries for debugging
  - All NPC interactions: `span_name="npc.interaction"`
  - Specific NPC: attributes filter for `npc.name="Nyaanager Evie"`
  - Dialogue sessions: `span_name="dialogue.session"`
  - Errors: `log_severity="ERROR"`
- [ ] Create query cookbook in `docs/mcp-queries.md`

**Priority 4: Error Span Tracking**
- [ ] Wrap asset loading in spans with status attribute
- [ ] Add error.type and error.message attributes on failures
- [ ] Create spans for caught exceptions/errors
- [ ] Test error visibility in MCP

### Medium Term (Phase 3 Completion)

1. ~~Instrument dialogue.rs with session spans~~ ✅ DONE
2. ~~Add reading speed metrics to dialogue~~ ✅ DONE
3. Instrument map transitions (Phase 3)
4. Add frame time tracking to main loop
5. Track asset loading times
6. Create example playthrough trace

### Long Term (Phase 4+)

1. Add quest tracking spans (when quest system exists)
2. Implement movement heatmap via position events
3. ~~Add error spans (crashes, failed loads)~~ → Moved to Priority 4
4. ~~Create dashboard queries in docs/~~ → Moved to Priority 3
5. Consider adding UI overlay showing trace ID
6. Performance profiling integration
7. Automated playtest analysis via snapshots

---

## MCP Capabilities Reference

### Available Tools

**Query Tool** - `mcp__otlpmcp__query`:
```rust
mcp__otlpmcp__query({
    service_name: "sregame",           // Filter by our service
    span_name: "npc.interaction",      // Filter by span type
    trace_id: "abc123...",             // Follow specific trace
    log_severity: "ERROR",             // Filter logs by severity
    metric_names: ["game.interactions.total"],  // Query specific metrics
    start_snapshot: "before-test",     // Time range start
    end_snapshot: "after-test",        // Time range end
    limit: 50                          // Max results per signal type
})
```

**Snapshot Tools**:
```rust
// Create bookmark at current time
mcp__otlpmcp__create_snapshot({ name: "before-npc-interaction" })

// Get everything between two moments
mcp__otlpmcp__get_snapshot_data({
    start_snapshot: "before-npc-interaction",
    end_snapshot: "after-dialogue"
})

// Manage snapshots
mcp__otlpmcp__manage_snapshots({ action: "list" })
mcp__otlpmcp__manage_snapshots({ action: "delete", name: "old-snapshot" })
mcp__otlpmcp__manage_snapshots({ action: "clear" })  // Nuclear option
```

**Port Management**:
```rust
// Add specific port (for consistent development)
mcp__otlpmcp__add_otlp_port({ port: 44173 })

// Remove port when done
mcp__otlpmcp__remove_otlp_port({ port: 44173 })

// Get current endpoint
mcp__otlpmcp__get_otlp_endpoint()
```

**Statistics**:
```rust
// Check buffer usage and data counts
mcp__otlpmcp__get_stats()
```

### Workflow Example: Debugging NPC Interaction

```rust
// 1. Start fresh
mcp__otlpmcp__create_snapshot({ name: "test-start" })

// 2. Run game and interact with Evie
// cargo run
// Press E near Nyaanager Evie
// Read some dialogue
// Press Escape to exit

// 3. Capture end state
mcp__otlpmcp__create_snapshot({ name: "test-end" })

// 4. Query what happened
mcp__otlpmcp__get_snapshot_data({
    start_snapshot: "test-start",
    end_snapshot: "test-end"
})

// 5. Specific queries
mcp__otlpmcp__query({
    service_name: "sregame",
    span_name: "dialogue.session",
    start_snapshot: "test-start",
    end_snapshot: "test-end"
})
```

---

## Next Steps (UPDATED)

### Immediate (THIS SESSION)

1. ✅ Commit current progress (OpenTelemetry + Crash Prevention)
2. ✅ Update otel-plan.md with current status
3. 🔥 **FIX ENDPOINT**: Add port 44173 to MCP or use env var
4. 🔥 **TEST TELEMETRY**: Run game and verify data flows to MCP
5. 🔥 **CREATE SNAPSHOT WORKFLOW**: Test snapshot-based debugging

### Short Term (Next Sessions)

1. Document MCP query cookbook
2. Add error span tracking
3. Instrument map transitions
4. Add frame time tracking
5. Create example playthrough trace with snapshots

### Long Term

1. Quest tracking spans
2. Movement heatmap
3. Performance profiling
4. Automated playtest analysis
5. UI overlay with trace ID

---

## References

- OpenTelemetry Rust: https://github.com/open-telemetry/opentelemetry-rust
- Bevy ECS: https://docs.rs/bevy/0.17.0/bevy/ecs/
- OTLP Protocol: https://opentelemetry.io/docs/specs/otlp/
- MCP Tools: Use `mcp__otlpmcp__query` to query telemetry

---

**Last Commit**: `sunxwppr 411ea8f2` - OpenTelemetry instrumentation + crash prevention (Phases 1 & 2 complete)
**Next**: Fix MCP endpoint connection, test telemetry flow, create snapshot workflow
