# Gemini Feedback Loop Improvements Plan

This document outlines the roadmap for improving the development feedback loop for AI agents (specifically Gemini) working on the SRE Game. The goal is to enable autonomous iteration, verification, and debugging without human intervention.

## 1. Telemetry & Observability (The "Eyes")

Current State: We have basic OTLP export for logs, traces, and metrics.
Missing: Granularity for short test runs and richer context for state verification.

### Feature Requests

#### 1.1. Configurable Metric Export Interval
*   **Problem:** Default OTLP export interval (10s) is too slow for short CI tests (3-5s). We miss performance metrics like `game.frame_time`.
*   **Solution:** Expose a configuration flag (e.g., `--otlp-interval-ms`) or env var to set the `PeriodicReader` interval.
*   **Target:** Allow intervals as low as 100ms for high-resolution updates during tests.

#### 1.2. Semantic State Tracing
*   **Problem:** `game_session` trace is too high-level. We need to know *where* the player is and *what* the state is without parsing text logs.
*   **Solution:** Add rich attributes to the `game_session` or periodic `player.state` spans:
    *   `player.map_id` (e.g., "Town of Endgame")
    *   `player.position` (x, y)
    *   `player.inventory` (list of items)
    *   `game.active_dialogue` (current node ID)

#### 1.3. Telemetry Snapshots (Done/Verify)
*   **Status:** The `otlp-mcp` tool `create_snapshot` works.
*   **Usage:** Agents should standardize on `create_snapshot "before_action"` -> `perform_action` -> `create_snapshot "after_action"` -> `get_snapshot_data`.

## 2. Control & Interaction (The "Hands")

Current State: `cargo run ... --seconds 5` runs the game, but agents can't interact dynamically.

### Feature Requests

#### 2.1. Bevy Remote Protocol (BRP) Driver
*   **Status:** ✅ **Implemented** (2026-07-12, via `bevy_brp_extras` + `--remote`/`--remote-port`)
*   **Problem:** Agents currently just "watch" the game run.
*   **What shipped:** `--remote` serves standard BRP (query/mutate/spawn)
    plus `brp_extras/send_keys` (walk Amy, press E, advance dialogue),
    `brp_extras/screenshot`, and `brp_extras/shutdown`. Plain `curl`
    JSON-RPC works; `bevy_brp_mcp` exposes the same as native MCP tools.
    Verified end-to-end (launch → screenshot → shutdown) under gamescope.
    See `docs/agents/AUTONOMY_GUIDE.md`. (An earlier draft claimed a
    `scripts/agent_drive.sh` existed — it never did; the guide's verified
    curl recipes replace it.)

## 3. Visual Data (The "Vision")

Current State: Headless mode provides no visual output. `screenshot.rs` examples require a window.

### Feature Requests

#### 3.1. The "Matrix View" (ASCII/Semantic Map)
*   **Status:** ✅ **Implemented** (via `SemanticViewportPlugin` in `src/viewport.rs`)
*   **Concept:** Agents don't need 4K pixels; they need spatial relationships.
*   **Implementation:** A system that runs every N frames (or on demand via BRP) and logs a representation of the viewport:
    ```
    [INFO] Viewport State (10x10):
    ..........
    ...P......  (P = Player)
    ...N......  (N = NPC)
    ..........
    ```
*   **Benefit:** Extremely cheap, parses easily, verifies logical rendering (e.g., "Is the NPC actually in front of the player?").

#### 3.2. Headless Screenshots (Long Term)
*   **Concept:** True render-to-disk without a window.
*   **Implementation:**
    *   Requires configuring `wgpu` with a texture target instead of a surface.
    *   Copy texture data to CPU buffer -> Encode to PNG -> Save to disk.
*   **Why:** To verify shader glitches, z-ordering issues, or asset loading failures that ASCII can't catch.
*   **Complexity:** High. Bevy 0.17 `ScheduleRunnerPlugin` doesn't setup the Render Graph by default.

## 4. Autonomous Test Suite

### 4.1. `agent_verify.sh`
A script specifically designed for agents to run:
1.  Starts `otlp-mcp` (if needed).
2.  Runs the game with BRP + OTLP.
3.  Executes a BRP script to walk to an NPC.
4.  Queries OTLP to verify the `dialogue.started` event occurred.
5.  Returns 0 (pass) or 1 (fail) with a summary.
