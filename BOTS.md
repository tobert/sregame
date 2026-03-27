# SREGame - Project Directives

## Project Overview

**sregame** (The Endgame of SRE) is a mini educational game that Amy Tobey
presented at SRECon NA 2022. It was originally written in RPGMaker MZ.
This version is written in Rust using the Bevy 0.17 game engine.

**The Endgame of SRE** teaches SRE principles through character interactions in a pixel art visual novel format.
- **No combat** - pure exploration and dialogue
- **Educational focus** - error budgets, SLOs, organizational culture, psychological safety
- **Original source**: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/`

## Technology

- **Language**: Rust 1.80+ with Bevy 0.17
- **Game Engine**: Bevy 0.17 (ECS architecture)
- **Asset Format**: Pixel art (48x48 tiles, 960x540 base, 2x upscaled to 1920x1080)
- **Key Dependencies**:
  - bevy 0.17 (game engine)
  - bevy_ecs_tilemap (tilemap rendering)
  - anyhow (error handling)
  - serde/serde_json (data serialization)
- **Version Control**: Jujutsu (jj) with GitHub integration

## High Level Game Features

1. Pixel art JRPG visual novel (no combat, dialogue-driven)
2. Visustella Fantasy Tiles MZ for tilesets and characters (licensed for reuse)
4. 48x48 tile size, 960x540 base resolution (2x upscaled to 1920x1080)

