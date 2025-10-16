# BOTS.md - Coding Agent Context for sregame

SRE Game is a mini educational game that Amy Tobey presented at SRECon NA 2022. It was originally
written in RPGMaker MZ. This version is written in Rust using the Bevy 0.17 game engine.

## Project Overview

**The Endgame of SRE** teaches SRE principles through character interactions in a pixel art visual novel format.
- **No combat** - pure exploration and dialogue
- **Educational focus** - error budgets, SLOs, organizational culture, psychological safety
- **Original source**: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/`

## High Level Game Features

1. Pixel art JRPG visual novel (no combat, dialogue-driven)
2. Visustella Fantasy Tiles MZ for tilesets and characters (licensed for reuse)
3. **NEVER use default RPG Maker assets** (only Visustella licensed content)
4. 48x48 tile size, 960x540 base resolution (2x upscaled to 1920x1080)

## Development Guidelines

**Error Handling**:
- Use `anyhow::Result` for all fallible operations
- Never use `unwrap()` - always propagate errors with `?`
- Add context with `.context()` for debugging
- Never silently discard errors with `let _ =`
- Handle reconnection gracefully on network failures

**Code Style**:
- Prioritize correctness and clarity over performance
- No organizational comments that summarize code
- Comments should only explain "why" when non-obvious
- Implement functionality in existing files unless it's a new logical component
- Avoid `mod.rs` files - use `src/module_name.rs` directly
- Use full words for variable names (no abbreviations)

**Bevy 0.17 Specific**:
- Use `ImagePlugin::default_nearest()` for pixel-perfect rendering (prevents blurry sprites)
- Bevy 0.17 uses required components - `Sprite` auto-includes `Transform` and `Visibility`
- Spawn cameras with `Camera2d` component directly (no bundles needed)
- For 2D games, camera Z must be 999.9
- Use `in_state()` run conditions for state-specific systems
- State management: `States` for major phases, `SubStates` for variations
- Text rendering: `Text2d` for world space, `Text` for UI

**Project Structure**:
- One plugin per major feature (PlayerPlugin, DialoguePlugin, etc.)
- Modules: `game_state.rs`, `player.rs`, `camera.rs`, `tilemap.rs`, `dialogue.rs`, `npc.rs`, `assets.rs`
- See `build-plan/00-overview.md` for complete architecture

## Git Commits

* Always review `git status` and `git diff` before committing
* Use `git add` precisely on individual files
* Claude should add `Co-authored-by: Claude <claude@anthropic.com>`
* Gemini should add `Co-authored-by: Gemini <gemini@google.com>`

## Build Plan

The `build-plan/` directory contains detailed implementation guides:
- Start with `build-plan/00-overview.md` for complete project roadmap
- Follow steps 01-09 sequentially (each builds on previous steps)
- Each step includes complete code examples, testing procedures, and success criteria
- Total MVP implementation time: 12-40 hours depending on experience

## Key References

- **Original game data**: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/`
- **Visustella assets**: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/`
- **Build plan**: `build-plan/00-overview.md`
- **Bevy version**: 0.17 (critical - examples are version-specific)

## Available Agents

When you need specialized help, use these agents:
- **bevy-expert**: Bevy 0.17 API questions, best practices, troubleshooting
- **endgame-sre-expert**: Original game content, dialogue extraction, asset identification
- **rust-bevy-consultant**: Advanced Rust patterns, Bevy ECS architecture decisions
