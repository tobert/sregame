# CONTEXT.md - Session Bridge

**Last Updated**: 2025-11-09 by Claude

## 🎮 Project Quick Reference

**What**: Educational SRE game (visual novel, no combat)
**Tech**: Rust + Bevy 0.17, pixel art (48x48 tiles)
**Status**: MVP functional, dialogue and NPC interaction working
**Source**: `/home/atobey/src/sregame/`

## 🔑 5 Key Facts for Next Agent

1. **Bevy 0.17 specifics**: Use `ImagePlugin::default_nearest()`, camera Z = 999.9, required components auto-included
2. **State system**: GameState (Loading/Playing/Dialogue) + Scene substates (TownOfEndgame/TeamMarathon)
3. **Error handling**: Always use `anyhow::Result`, never `unwrap()`, add `.context()` for debugging
4. **Asset structure**: Licensed Visustella content only, never default RPGMaker assets
5. **Build plan**: Steps 01-02, 06-07, 09 complete; Steps 03-05, 08 pending (animations, sound, UI, portals)

## 📂 Critical File Locations

```
src/
├── main.rs              # App setup, plugin registration
├── game_state.rs        # State management, debug keys
├── player.rs            # Player movement, input
├── camera.rs            # Camera follow with bounds
├── tilemap.rs           # Map loading from JSON
├── dialogue.rs          # Dialogue system, typewriter
└── npc.rs               # NPC spawning, interaction

assets/
├── textures/            # Sprite sheets, tilesets
├── fonts/               # Text rendering
└── data/
    ├── maps/            # *.map.json
    └── dialogue/        # *.dialogue.json

build-plan/              # Implementation guides
docs/agents/             # Memory system (you are here)
```

## 🧪 How to Test

```bash
cargo run                    # Start game
# Arrow keys: Move player
# E: Interact with nearby NPC
# Space: Advance dialogue
# ESC: Exit dialogue / debug key changes
# D: (removed) Test dialogue trigger
```

## 🚦 Current State Summary

**Working:**
- Player movement with keyboard
- Camera following player with bounds
- Tilemap rendering from JSON
- NPC proximity detection
- Dialogue system with typewriter effect
- State transitions (Playing ↔ Dialogue)

**Not Yet Implemented:**
- Character animations (walk cycles)
- Sound effects and music
- Pause menu / save system
- Map portals (scene transitions)
- Quest/objective tracking
- Additional maps beyond Town of Endgame

## 🎯 Immediate Next Tasks (Pick Any)

1. **Step 03**: Implement player sprite animations (4-direction walk cycles)
2. **Step 04**: Add sound effects and background music
3. **Step 05**: Create pause menu and UI systems
4. **Step 08**: Implement portal system for map transitions
5. **Content**: Port more dialogue from original game
6. **Polish**: Improve dialogue text wrapping algorithm

## 🤝 Handoff Checklist

Before ending your session:
- [ ] Update NOW.md with current state
- [ ] Add new patterns to PATTERNS.md if discovered
- [ ] Update this CONTEXT.md with any critical changes
- [ ] Use `jj describe` to document your work
- [ ] Run `cargo check` and `cargo test` to verify stability
- [ ] Note any blockers or questions for next agent

## 💡 Tips for Success

1. **Read CLAUDE.md first** - Essential guidelines and patterns
2. **Check build-plan/** - Detailed implementation steps with examples
3. **Use bevy-expert agent** - For Bevy 0.17 API questions
4. **Use endgame-sre-expert agent** - For original game content extraction
5. **Always test after changes** - Game should build and run
6. **Respect the asset license** - Visustella content only

## 🔗 Related Resources

- Original game: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/`
- Bevy 0.17 docs: https://docs.rs/bevy/0.17.0/
- Build plan: `build-plan/00-overview.md`
- Global instructions: `~/.claude/CLAUDE.md`

---
*Update this file when context significantly changes or before handoff*
