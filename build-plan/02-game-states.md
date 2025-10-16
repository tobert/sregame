# Build Plan 02: Game State Management

## Objective

Implement a robust state management system using Bevy 0.17's `States` and `SubStates` APIs. This provides the foundation for transitioning between Loading, Playing, and Dialogue modes.

## Context

The SRE Game needs clear separation between different gameplay phases:
- **Loading**: Asset loading phase (shows progress, transitions to Playing when done)
- **Playing**: Main exploration mode (player walks around, can interact with NPCs)
- **Dialogue**: Conversation mode (player reads NPC dialogue, advances with Space/Enter)

Bevy 0.17 provides:
- `States` for major game phases
- `SubStates` for variations within a state
- `OnEnter`, `OnExit`, `OnTransition` schedules for state transitions
- `in_state()` run conditions for state-specific systems

## Prerequisites

- Completed: **01-project-setup.md**
- Working `src/main.rs` with Bevy app initialized
- Cargo.toml has `bevy = "0.17"`

## Tasks

### 1. Create game_state.rs Module

Create `src/game_state.rs`:

```rust
use bevy::prelude::*;

/// Primary game state controlling the major phases
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum GameState {
    /// Loading assets, shows progress UI
    #[default]
    Loading,
    /// Active gameplay - player can move and interact
    Playing,
    /// Dialogue is active - player reads text and advances
    Dialogue,
}

/// Current scene/map location (substate of Playing)
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, SubStates)]
#[source(GameState = GameState::Playing)]
pub enum Scene {
    /// Hub area connecting all team locations
    #[default]
    TownOfEndgame,
    /// Team Marathon building interior
    TeamMarathon,
}

/// Plugin that manages game state transitions
pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<Scene>()
            .add_systems(Update, (
                debug_state_changes,
                handle_escape_key,
            ));
    }
}

/// Logs state transitions for debugging
fn debug_state_changes(
    state: Res<State<GameState>>,
) {
    if state.is_changed() {
        info!("Game state changed to: {:?}", state.get());
    }
}

/// Handle Escape key to return from Dialogue to Playing
fn handle_escape_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match current_state.get() {
            GameState::Dialogue => {
                info!("Exiting dialogue mode");
                next_state.set(GameState::Playing);
            }
            _ => {}
        }
    }
}
```

**Key Design Decisions**:
- `#[default]` on `Loading`: Game always starts by loading assets
- `Scene` as a `SubState`: Only active when `GameState::Playing`
- Escape key exits dialogue (common pattern in visual novels)
- Debug logging for all state changes

### 2. Update main.rs to Use States

Modify `src/main.rs` to integrate the state plugin:

```rust
use bevy::prelude::*;

mod game_state;
use game_state::{GameState, GameStatePlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
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
        .add_plugins(GameStatePlugin)
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    info!("SRE Game initialized");
}

/// Called when entering Loading state
fn on_enter_loading() {
    info!("Entering Loading state");
    // Asset loading will be implemented in step 08
}

/// Called when entering Playing state
fn on_enter_playing() {
    info!("Entering Playing state - player can explore");
}

/// Called when entering Dialogue state
fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}
```

**Integration Points**:
- `GameStatePlugin` added to plugin list
- `OnEnter(state)` schedules run once when state begins
- Placeholder functions for future implementations

### 3. Add Temporary State Transition for Testing

For now, add a test system to verify state transitions work:

Add this to `src/main.rs`:

```rust
use bevy::prelude::*;

mod game_state;
use game_state::{GameState, GameStatePlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
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
        .add_plugins(GameStatePlugin)
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        // TEMPORARY: Test state transitions
        .add_systems(Update, test_state_transitions.run_if(in_state(GameState::Playing)))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    info!("SRE Game initialized");
}

fn on_enter_loading(mut next_state: ResMut<NextState<GameState>>) {
    info!("Entering Loading state");
    // TEMPORARY: Immediately transition to Playing for testing
    // In step 08, this will wait for assets to load
    next_state.set(GameState::Playing);
}

fn on_enter_playing() {
    info!("Entering Playing state - player can explore");
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}

/// TEMPORARY: Press D to test dialogue state transition
fn test_state_transitions(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        info!("Testing transition to Dialogue state");
        next_state.set(GameState::Dialogue);
    }
}
```

**Testing Flow**:
1. App starts in `Loading` state
2. Immediately transitions to `Playing` state
3. Press `D` to transition to `Dialogue` state
4. Press `Escape` to return to `Playing` state

### 4. Verify State System

Run the application and test:

```bash
cargo run
```

Expected behavior:
- Console shows: "Entering Loading state"
- Console shows: "Entering Playing state - player can explore"
- Press `D` key → Console shows: "Entering Dialogue state - reading conversation"
- Press `Escape` key → Console shows: "Entering Playing state - player can explore"

Check console logs for state transition messages.

## Success Criteria

- [ ] `src/game_state.rs` created with `GameState` and `Scene` enums
- [ ] `GameStatePlugin` properly integrated into `main.rs`
- [ ] State transitions work: Loading → Playing → Dialogue → Playing
- [ ] Escape key exits dialogue mode
- [ ] Console logs show all state changes
- [ ] No compilation errors or warnings

## State Lifecycle Reference

```
Loading (OnEnter: immediate transition to Playing)
   ↓
Playing (OnEnter: player spawned, camera active)
   ├─→ Dialogue (OnEnter: dialogue box shown)
   │      ├─→ [Escape] → Playing
   │      └─→ [Dialogue ends] → Playing
   └─→ Scene::TownOfEndgame (default)
       └─→ Scene::TeamMarathon (when player enters door)
```

## Next Steps

After completing this task:
1. **03-player-system.md**: Player movement will use `in_state(GameState::Playing)`
2. **06-dialogue-system.md**: Dialogue UI will spawn in `OnEnter(GameState::Dialogue)`
3. **08-asset-loading.md**: Loading state will wait for assets before transitioning

## Notes for Implementation

- The `Scene` substate is currently unused - it will be activated in step 05 (tilemap rendering)
- State transitions are instant - no fade effects yet (can be added later)
- All systems should use `run_if(in_state(...))` to prevent running in wrong states
- Remove `test_state_transitions` function after completing step 06 (dialogue system)

## Reference

- Bevy States Guide: https://bevyengine.org/learn/quick-start/next-steps/states/
- Example code from bevy-expert agent consultation
