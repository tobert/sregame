# The Endgame of SRE

A short educational game teaching SRE principles through a pixel art visual novel experience.

![Overhead pixel-art view of the Town of Endgame: Amy's player character stands at the edge of a terracotta-brick plaza ringed by stone paths, lamp posts, flowering trees, and a lily pond. A wild-haired NPC waits on the path nearby, and a blue-roofed brick storefront anchors the east side of the square.](screenshots/town-plaza.png)

Originally presented at [QCon SF 2022](https://qconsf.com/speakers/amytobey)
and the [SREcon23 Americas keynote](https://www.youtube.com/watch?v=BEs6j-BOl20). Originally
built with RPGMaker MZ, this version is a rewrite in Rust using the Bevy game engine (0.19).

The game was intended for a single talk following a single well-practiced path, so there will
be some weird interactions if you do not follow that exact path. The main example being the
inn, if you interact with the characters around the table they have their old dialog. If you
interact with the map at the table you get the dialog that was presented.

## About

**The Endgame of SRE** is a dialogue-driven exploration game (no combat) that teaches Site Reliability Engineering concepts through character interactions:

- **Error budgets** and service level objectives (SLOs)
- **Organizational culture** and team dynamics
- **Psychological safety** in engineering teams
- **SRE best practices** in a story-driven format

**Technical Stack:**
- Engine: Bevy 0.19 (Rust game engine)
- Graphics: Pixel art JRPG style (48x48 tiles, 960x540 resolution)
- Assets: Visustella Fantasy Tiles MZ (licensed)

## Screenshots

![The town inn, a broad orange-roofed building with a wooden sign reading INN hanging over its door. A black-and-tan dog stands in the grass out front, and stone paths wind past a well and picket fences toward the plaza.](screenshots/inn.png)

![A dialogue box covers the lower third of the screen at presentation scale: a large portrait of Paws Alljohn, a stern character with white hair and a gold-armored visor, beside his line: Haha not really. These folks have the best SREs and infrastructure money can buy, and they still have incidents! Behind the box, the town square with NPCs on the flowered paths.](screenshots/paws-alljohn-dialogue.png)

![Inside Team Disconnect: two wood-paneled shop rooms with bookshelves, a crystal ball on a red-draped table, and boots and armor on display shelves. Amy stands face to face with Managear Greg, an NPC in a goggled aviator helmet, in the central hallway. His dialogue box reads: My people work so hard and it feels like we just cant win. I really hope this retro will help.](screenshots/managear-greg-dialogue.png)

## Quick Start

### Prerequisites

- Rust toolchain (2024 edition)
- For Windows builds: `cargo xwin` (`cargo install cargo-xwin`)

### Play on Linux

```bash
cargo run
```

### Building for the Web (WebAssembly)

The game runs in the browser via `wasm32-unknown-unknown`, built with the
[Bevy CLI](https://thebevyflock.github.io/bevy_cli/) (note: install from git —
the `bevy_cli` name on crates.io is only a reservation):

```bash
rustup target add wasm32-unknown-unknown
cargo install --git https://github.com/TheBevyFlock/bevy_cli --locked bevy_cli

# Serve locally at http://127.0.0.1:4000 (dev profile, fast iteration)
bevy run web

# Deployable static bundle (wasm-opt'd) in target/bevy_web/web-release/sregame/
bevy build --release web --bundle
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

## License

See `LICENSE` file.

## References

- SREcon23 Americas keynote recording: https://www.youtube.com/watch?v=BEs6j-BOl20
- USENIX presentation page: https://www.usenix.org/conference/srecon23americas/presentation/tobey
- Bevy Engine: https://bevyengine.org
- VisuStella MZ Sample Game Project: https://visustella.itch.io/visumz-sample
