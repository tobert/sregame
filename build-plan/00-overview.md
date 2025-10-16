# SRE Game MVP - Build Plan Overview

## Project Summary

**The Endgame of SRE** is a pixel art JRPG visual novel that teaches SRE principles through character interactions. This is a Bevy 0.17 rewrite of the original RPGMaker MZ presentation game by Amy Tobey from SRECon NA 2022.

**Core Concept**: No combat, pure exploration and dialogue. Players walk through different team areas and learn about organizational culture, error budgets, SLOs, and healthy SRE practices through NPC conversations.

## MVP Scope

The MVP delivers a vertical slice containing:
- **2 Maps**: Town of Endgame (hub) and Team Marathon interior
- **5 NPCs**: Team Marathon members teaching SRE concepts
- **Full dialogue system**: Typewriter effect, character portraits, proper UI
- **Complete player experience**: Walk around, interact with NPCs, learn SRE principles

## Technology Stack

- **Engine**: Bevy 0.17
- **Language**: Rust (2021 edition)
- **Key Crates**:
  - `bevy = "0.17"` - Game engine
  - `bevy_ecs_tilemap = "0.17"` - Tilemap rendering
  - `anyhow = "1.0"` - Error handling
  - `serde/serde_json` - Data loading

## Build Plan Structure

This build plan is divided into 9 sequential steps, each with a detailed implementation guide:

### 01. Project Setup
**File**: `01-project-setup.md`
**Objective**: Initialize Bevy 0.17 project with proper dependencies and directory structure
**Duration**: ~30 minutes
**Key Outputs**:
- Working Cargo project
- Asset directory structure
- Basic window and camera

### 02. Game State Management
**File**: `02-game-states.md`
**Objective**: Implement state machine for Loading → Playing → Dialogue flow
**Duration**: ~1 hour
**Key Outputs**:
- `GameState` enum (Loading, Playing, Dialogue)
- `Scene` substate (TownOfEndgame, TeamMarathon)
- State transition systems

### 03. Player Character System
**File**: `03-player-system.md`
**Objective**: Player sprite, 8-directional movement, walk animation
**Duration**: ~2 hours
**Key Outputs**:
- Amy character sprite rendering
- WASD/Arrow key movement
- Direction-based animation
- Velocity system

### 04. Camera System
**File**: `04-camera-system.md`
**Objective**: Smooth camera following player with map bounds
**Duration**: ~1 hour
**Key Outputs**:
- Camera smoothly tracks player
- Configurable follow speed
- Boundary clamping

### 05. Tilemap Rendering
**File**: `05-tilemap-rendering.md`
**Objective**: Render game maps with bevy_ecs_tilemap, implement collision
**Duration**: ~3 hours
**Key Outputs**:
- Tilemap rendering system
- Collision detection
- Map centering and camera bounds integration

### 06. Dialogue System
**File**: `06-dialogue-system.md`
**Objective**: UI for conversations with typewriter effect and portraits
**Duration**: ~3 hours
**Key Outputs**:
- Dialogue box UI at bottom of screen
- Typewriter text animation
- Character portrait display
- Two-stage advance (skip typing → advance line)

### 07. NPC Interactions
**File**: `07-npc-interactions.md`
**Objective**: Proximity-based NPC interaction triggers
**Duration**: ~2 hours
**Key Outputs**:
- NPC spawning system
- Proximity detection (InRange marker)
- E-key interaction
- Integration with dialogue system

### 08. Asset Loading
**File**: `08-asset-loading.md`
**Objective**: Loading screen that waits for all assets
**Duration**: ~2 hours
**Key Outputs**:
- `GameAssets` resource with all handles
- Loading progress tracking
- Loading screen UI
- Centralized asset management

### 09. Content Port
**File**: `09-content-port.md`
**Objective**: Import actual map data and dialogue from RPGMaker JSON
**Duration**: ~4 hours
**Key Outputs**:
- RPGMaker JSON parsing
- Real map data loading (Map002, Map004)
- NPC positions from event data
- Actual dialogue text

## Execution Order

**Important**: These steps must be completed in sequence. Each step builds on the previous ones.

```
Start
  ↓
01-project-setup.md ← Set up Bevy project
  ↓
02-game-states.md ← Add state management
  ↓
03-player-system.md ← Player movement
  ↓
04-camera-system.md ← Camera following
  ↓
05-tilemap-rendering.md ← Map rendering
  ↓
06-dialogue-system.md ← Dialogue UI
  ↓
07-npc-interactions.md ← NPC system
  ↓
08-asset-loading.md ← Asset management
  ↓
09-content-port.md ← Port real content
  ↓
MVP Complete!
```

## Total Estimated Time

- **Experienced Rust/Bevy developer**: 12-16 hours
- **Intermediate developer**: 20-25 hours
- **Learning Bevy from scratch**: 30-40 hours

## Testing Milestones

After each step, you should be able to run the game and see incremental progress:

| Step | What You Should See |
|------|---------------------|
| 01 | Black window with title "The Endgame of SRE" |
| 02 | Console logs showing state transitions |
| 03 | Player sprite moving with WASD keys |
| 04 | Camera following player smoothly |
| 05 | Map rendered, player blocked by walls |
| 06 | Press D to see dialogue box with test text |
| 07 | Walk to NPC, press E, see their dialogue |
| 08 | Loading screen before gameplay starts |
| 09 | Actual game map and NPC dialogue from original |

## Key Files Created

By the end of the build plan, your project will have:

```
src/
├── main.rs              - App setup, plugin registration
├── game_state.rs        - State management
├── player.rs            - Player character and movement
├── camera.rs            - Camera following system
├── tilemap.rs           - Map rendering and collision
├── dialogue.rs          - Dialogue UI and typewriter effect
├── npc.rs               - NPC spawning and interaction
├── assets.rs            - Asset loading and management
└── rpgmaker_data.rs     - RPGMaker JSON parsing

assets/
├── textures/
│   ├── characters/      - Player and NPC sprites
│   ├── tilesets/        - Map tiles (Visustella)
│   └── portraits/       - Character portraits for dialogue
├── fonts/
│   └── dialogue.ttf     - UI font
└── data/
    ├── maps/            - (Future) Processed map data
    └── dialogue/        - (Future) Dialogue JSON files
```

## Common Issues and Solutions

### Issue: Compilation errors about missing types
**Solution**: Make sure you're on the correct step. Earlier steps may reference future modules.

### Issue: Assets not loading (pink checkerboard)
**Solution**:
1. Verify asset files are in correct locations
2. Check paths in code match actual file locations
3. Wait for step 08 before expecting clean asset loading

### Issue: Player can't move
**Solution**: Check that player movement system uses `run_if(in_state(GameState::Playing))`

### Issue: Dialogue doesn't appear
**Solution**: Verify you've completed step 06 before step 07. NPCs need dialogue system to work.

### Issue: Map doesn't render
**Solution**: Check that `bevy_ecs_tilemap` plugin is added to app

## Development Best Practices

Throughout implementation, follow these principles from CLAUDE.md:

1. **Error Handling**: Always use `anyhow::Result`, never `unwrap()`
2. **No organizational comments**: Code should be self-documenting
3. **Incremental testing**: Run `cargo run` after each step
4. **Git commits**: Commit after completing each build plan step
5. **Full words**: No abbreviations in variable names

## MVP Success Criteria

The MVP is complete when:

- [ ] Player spawns in Town of Endgame hub
- [ ] Can walk around the town map
- [ ] Can enter Team Marathon building
- [ ] Can interact with 5 NPCs in Team Marathon
- [ ] Each NPC has multiple dialogue lines
- [ ] Dialogue shows character portraits
- [ ] Typewriter effect plays smoothly
- [ ] Can navigate back to town hub
- [ ] All assets load without errors
- [ ] No pink/magenta missing textures
- [ ] No compilation warnings

## Post-MVP Enhancements

After completing the MVP, consider adding:

1. **More maps**: Team Disco, Team Inferno, Mahogany Row
2. **Map transitions**: Doors that change scenes
3. **More NPCs**: All characters from original game
4. **Dialogue branches**: Different responses based on choices
5. **Metrics dashboard**: Track player learning progress
6. **Audio system**: Background music per map
7. **Save system**: Remember progress between sessions
8. **Polish**: Fade transitions, particle effects, UI animations

## Learning Resources

If you get stuck:

- **Bevy 0.17 Book**: https://bevyengine.org/learn/book/
- **Bevy Examples**: https://github.com/bevyengine/bevy/tree/main/examples
- **Bevy Discord**: https://discord.gg/bevy
- **bevy_ecs_tilemap docs**: https://docs.rs/bevy_ecs_tilemap/

## Original Game Reference

- **Source files**: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/`
- **Map data**: `data/Map*.json`
- **Assets**: `img/` directory (Visustella Fantasy Tiles)
- **Presentation video**: Search "Endgame of SRE SRECon 2022"

## Project Context

This game was originally created as an educational tool for SRECon NA 2022. It teaches:
- **Westrum organizational culture model**: Generative vs pathological
- **Error budgets and SLOs**: Practical implementation
- **Psychological safety**: Team health and sustainability
- **Incident response**: Post-mortems and learning culture
- **Work-life balance**: Avoiding burnout

The rewrite in Bevy makes it:
- More maintainable (Rust vs JavaScript)
- Cross-platform (no RPGMaker runtime)
- Extensible (full control over systems)
- Open source friendly (no proprietary tools)

## Getting Help

If you encounter issues not covered in the build plan:

1. Check the specific step's "Known Issues" section
2. Review error messages carefully - Rust compiler is helpful
3. Use `cargo build` to see all compilation errors at once
4. Add debug logging: `info!()`, `debug!()`, `warn!()`
5. Use Bevy's built-in diagnostics: `LogDiagnosticsPlugin`

## Final Notes

- **Stay sequential**: Don't skip steps or implement out of order
- **Test frequently**: Run the game after every major change
- **Commit often**: Use git to save progress at each milestone
- **Read carefully**: Each build plan has important context and rationale
- **Have fun**: You're building an educational game teaching real SRE principles!

Good luck building The Endgame of SRE MVP! 🚀
