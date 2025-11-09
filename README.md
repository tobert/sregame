# The Endgame of SRE

A Bevy-based educational game teaching SRE principles through a pixel art visual novel experience.

Originally presented at SRECon NA 2022 and built with RPGMaker MZ, this version is a complete rewrite in Rust using the Bevy 0.17 game engine.

## About

**The Endgame of SRE** is a dialogue-driven exploration game (no combat) that teaches Site Reliability Engineering concepts through character interactions:

- **Error budgets** and service level objectives (SLOs)
- **Organizational culture** and team dynamics
- **Psychological safety** in engineering teams
- **SRE best practices** in a story-driven format

**Technical Stack:**
- Engine: Bevy 0.17 (Rust game engine)
- Graphics: Pixel art JRPG style (48x48 tiles, 960x540 resolution)
- Assets: Visustella Fantasy Tiles MZ (licensed)

## Quick Start

### Play on Linux

```bash
cargo run
```

### Play on Windows (Cross-Compiled from Linux)

Build the Windows version from Linux/WSL:

```bash
./build-windows.sh
```

The executable will be at `target/x86_64-pc-windows-msvc/debug/sregame.exe`

## Development

### Prerequisites

- Rust toolchain (2024 edition)
- For Windows builds: `cargo xwin` (`cargo install cargo-xwin`)
- For automatic sync: HalfRemembered Launcher

### Building Locally

```bash
# Debug build (faster compilation)
cargo build

# Run directly
cargo run

# Release build (better performance)
cargo build --release
cargo run --release
```

### Cross-Compiling for Windows

From Linux or WSL:

```bash
# Install cargo-xwin if not already installed
cargo install cargo-xwin

# Build Windows executable + copy required DLLs
./build-windows.sh
```

The script will:
1. Cross-compile using `cargo xwin` for the `x86_64-pc-windows-msvc` target
2. Detect required DLLs using `objdump`
3. Copy necessary runtime libraries to the build directory

Output: `target/x86_64-pc-windows-msvc/debug/sregame.exe` (and `.dll` files)

## Automatic Sync with HalfRemembered Launcher

This project is configured to automatically sync builds and assets to Windows clients using [HalfRemembered Launcher](https://github.com/atobey/halfremembered-launcher), an SSH-based RPC system for pushing files to remote machines.

### Setup

1. **Start the HalfRemembered server** (on your build machine):

   ```bash
   # In the halfremembered-launcher directory
   cargo run --bin halfremembered-launcher server --port 20222
   ```

2. **Start the client daemon** (on your Windows machine):

   ```bash
   # Connect to the server
   halfremembered-launcher client --server your-build-machine:20222
   ```

3. **Configure filesystem watching** (on your build machine):

   ```bash
   # From the sregame directory
   halfremembered-launcher config-sync --server localhost:20222
   ```

   This uses the `.hrlauncher.toml` config to set up automatic syncing.

### How It Works

The `.hrlauncher.toml` configuration defines two sync targets:

1. **Windows Binaries** - Syncs the .exe and .dll files to Windows clients
   - Watches: `target/x86_64-pc-windows-msvc/debug/sregame.exe` and `*.dll`
   - Destination: Current directory on Windows client
   - Target: Only `windows-*` clients

2. **Game Assets** - Syncs textures, fonts, and data files
   - Watches: `assets/**/*` (recursive)
   - Excludes: Source files like `.psd`, `.blend`
   - Destination: `assets/` directory on client
   - Mirror mode: Keeps assets directory clean (deletes removed files)

### Development Workflow

Once configured, your workflow becomes:

```bash
# 1. Edit code and assets on Linux
vim src/player.rs
gimp assets/player.png

# 2. Build for Windows
./build-windows.sh

# 3. Files automatically sync to Windows client
#    (HalfRemembered detects changes and pushes immediately)

# 4. Run on Windows
#    The client receives files and you can test immediately
```

No manual copying, no rsync scripts, no waiting!

### Manual Sync (Alternative)

If you prefer manual syncing or don't want to use HalfRemembered Launcher:

```bash
# Edit sync-to-windows.sh with your Windows machine's IP
vim sync-to-windows.sh

# Run manual sync via rsync
./sync-to-windows.sh
```

## Project Structure

```
sregame/
├── src/
│   ├── main.rs           # Entry point and app setup
│   ├── game_state.rs     # Game state management
│   ├── player.rs         # Player movement and controls
│   ├── camera.rs         # Camera follow system
│   ├── tilemap.rs        # Tilemap rendering
│   ├── dialogue.rs       # Dialogue system
│   ├── npc.rs            # NPC interactions
│   └── assets.rs         # Asset loading
├── assets/               # Game assets (textures, fonts, etc.)
├── build-plan/           # Implementation guides (see 00-overview.md)
├── .hrlauncher.toml      # HalfRemembered Launcher sync config
├── build-windows.sh      # Cross-compile for Windows
└── sync-to-windows.sh    # Manual rsync script (legacy)
```

## Build Plan

Detailed implementation guides are available in `build-plan/`:

1. Start with `build-plan/00-overview.md` for the complete roadmap
2. Follow steps 01-09 sequentially
3. Each step includes code examples, testing procedures, and success criteria

Estimated time: 12-40 hours depending on Rust/Bevy experience

## Contributing

This is an educational project. See `BOTS.md` for coding guidelines and AI agent context.

**Key Guidelines:**
- Use `anyhow::Result` for error handling (never `unwrap()`)
- Follow Bevy 0.17 best practices (required components, state management)
- Prioritize clarity over performance
- Add `Co-authored-by` lines to commits when working with AI assistants

## License

See `LICENSE` file.

## References

- Original RPGMaker version: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/`
- SRECon NA 2022 presentation: [Link TBD]
- Bevy Engine: https://bevyengine.org
- HalfRemembered Launcher: https://github.com/atobey/halfremembered-launcher
