# Build Plan 01: Project Setup and Bevy 0.17 Initialization

## Objective

Set up a new Bevy 0.17 project with proper dependencies, project structure, and configuration for a pixel art 2D game. This establishes the foundation for the entire SRE Game MVP.

## Context

The SRE Game is a pixel art JRPG visual novel built with Bevy 0.17. It requires:
- Pixel-perfect rendering (no blurring on upscaled sprites)
- 960x540 base resolution (2x upscale to 1920x1080)
- 48x48 tile size for maps
- Error handling with `anyhow::Result` throughout (never use `unwrap()`)
- Clean project structure with one plugin per major feature

## Prerequisites

- Rust toolchain installed (1.70+)
- Working directory: `/home/atobey/src/sregame`
- No existing Cargo.toml (fresh project)

## Tasks

### 1. Initialize Cargo Project

If `Cargo.toml` doesn't exist, initialize the project:

```bash
cargo init --name sregame
```

### 2. Configure Cargo.toml

Create or update `Cargo.toml` with the following dependencies:

```toml
[package]
name = "sregame"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = "0.17"
bevy_ecs_tilemap = "0.17"
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Faster compile times during development
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

**Rationale**:
- `bevy = "0.17"`: Core game engine
- `bevy_ecs_tilemap`: Efficient tilemap rendering for RPG maps
- `anyhow`: Error handling with context (required by CLAUDE.md guidelines)
- `serde/serde_json`: For loading dialogue and NPC data from JSON files
- Dev profile optimizations: Balance compile time vs runtime performance

### 3. Create Project Directory Structure

Create the following directory structure:

```
src/
├── main.rs
├── game_state.rs      (will be created in step 02)
├── assets.rs          (will be created in step 08)
├── player.rs          (will be created in step 03)
├── camera.rs          (will be created in step 04)
├── dialogue.rs        (will be created in step 06)
├── npc.rs             (will be created in step 07)
└── tilemap.rs         (will be created in step 05)

assets/
├── textures/
│   ├── characters/    (sprite sheets)
│   ├── tilesets/      (Visustella tiles)
│   └── portraits/     (character faces for dialogue)
├── fonts/
│   └── dialogue.ttf   (UI font)
└── data/
    ├── maps/          (tilemap JSON data)
    └── dialogue/      (conversation JSON files)
```

Commands:
```bash
mkdir -p assets/textures/characters assets/textures/tilesets assets/textures/portraits
mkdir -p assets/fonts assets/data/maps assets/data/dialogue
```

### 4. Write Initial main.rs

Create `src/main.rs` with basic Bevy 0.17 setup:

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest()) // CRITICAL: Pixel-perfect rendering
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "The Endgame of SRE".to_string(),
                        resolution: (1920.0, 1080.0).into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                })
        )
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Spawn 2D camera
    commands.spawn(Camera2d);

    info!("SRE Game initialized");
}
```

**Key Points**:
- `ImagePlugin::default_nearest()`: Prevents texture filtering, keeping pixel art crisp
- Window resolution 1920x1080 (2x scale of 960x540 game resolution)
- `Camera2d` uses Bevy 0.17's simplified component syntax (no bundle needed)
- `resizable: false`: Maintains pixel-perfect rendering

### 5. Verify Build

Test that the project compiles and runs:

```bash
cargo build
cargo run
```

Expected output:
- A black window titled "The Endgame of SRE" at 1920x1080
- Console log: "SRE Game initialized"
- No compilation errors or warnings

### 6. Create .gitignore (if not exists)

Ensure `.gitignore` includes:

```gitignore
/target
Cargo.lock
*.swp
*.swo
*~
.DS_Store
```

## Success Criteria

- [ ] `cargo build` completes without errors
- [ ] `cargo run` opens a window with title "The Endgame of SRE"
- [ ] Directory structure created under `assets/`
- [ ] `src/main.rs` has pixel-perfect image plugin configured
- [ ] Project follows error handling guidelines (anyhow available, no unwrap)

## Next Steps

After completing this task:
1. Proceed to **02-game-states.md** to implement state management
2. The camera system is minimal here; full camera following comes in **04-camera-system.md**
3. Asset loading system will be added in **08-asset-loading.md**

## Reference Files

- CLAUDE.md guidelines (in project root)
- Bevy 0.17 docs: https://docs.rs/bevy/0.17.0/bevy/

## Notes for Implementation

- Never use `unwrap()` - always propagate errors with `?` or handle explicitly
- No `mod.rs` files - use `src/module_name.rs` directly
- Comments should explain "why", not "what"
- Test each change incrementally before moving to the next step
