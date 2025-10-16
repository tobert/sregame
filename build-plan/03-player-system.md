# Build Plan 03: Player Character System

## Objective

Implement the player character with sprite rendering, 8-directional movement, and sprite animation. Amy Tobey is the player character who walks through the game world to interact with NPCs.

## Context

**Player Character**: Amy Tobey
- Sprite sheet: `Amy-Walking.png` from original game
- Movement: 8-directional (N, NE, E, SE, S, SW, W, NW)
- Speed: ~100-150 pixels/second for comfortable exploration
- Animation: Walking animation cycles when moving, idle when stopped
- Scale: 2x upscale from base sprite size

The original RPGMaker game uses character sprites with this layout:
- Each character sheet has multiple rows (one per direction)
- Each row has 3 frames of walk animation
- Standard RPGMaker format: Down, Left, Right, Up (4 rows)

## Prerequisites

- Completed: **01-project-setup.md**, **02-game-states.md**
- `GameState` enum available in `src/game_state.rs`
- Camera2d spawned in main.rs

## Tasks

### 1. Create player.rs Module

Create `src/player.rs`:

```rust
use bevy::prelude::*;
use crate::game_state::GameState;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_player)
            .add_systems(Update, (
                player_movement_input,
                apply_movement,
                animate_player,
            ).chain().run_if(in_state(GameState::Playing)));
    }
}

/// Marker component for the player entity
#[derive(Component)]
pub struct Player;

/// Player's velocity in pixels per second
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// Player's current facing direction
#[derive(Component, Default)]
pub enum Facing {
    #[default]
    Down,
    Left,
    Right,
    Up,
}

impl Facing {
    /// Get the sprite row index for this direction
    fn sprite_row(&self) -> usize {
        match self {
            Facing::Down => 0,
            Facing::Left => 1,
            Facing::Right => 2,
            Facing::Up => 3,
        }
    }
}

/// Animation state for the player sprite
#[derive(Component)]
pub struct AnimationState {
    pub frame_timer: Timer,
    pub current_frame: usize,
    pub is_moving: bool,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            frame_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
            current_frame: 1, // Start at middle frame (idle pose)
            is_moving: false,
        }
    }
}

const PLAYER_SPEED: f32 = 150.0; // pixels per second

fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // Load Amy's sprite sheet
    let texture = asset_server.load("textures/characters/Amy-Walking.png");

    // Define sprite sheet layout: 3 frames per direction, 4 directions
    // Assuming each frame is 32x32 pixels (adjust based on actual sprite size)
    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(32, 32), // tile size - VERIFY THIS with actual sprite
        3,                   // columns (3 walk frames)
        4,                   // rows (4 directions)
        None,                // no padding
        None,                // no offset
    );
    let atlas_layout = texture_atlas_layouts.add(layout);

    commands.spawn((
        Player,
        Velocity(Vec2::ZERO),
        Facing::default(),
        AnimationState::default(),
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: 1, // Start with middle frame (row 0, frame 1)
            },
        ),
        Transform::from_xyz(0.0, 0.0, 1.0)
            .with_scale(Vec3::splat(2.0)), // 2x upscale for pixel art
    ));

    info!("Player (Amy) spawned at origin");
}

fn player_movement_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Velocity, &mut Facing, &mut AnimationState), With<Player>>,
) {
    let Ok((mut velocity, mut facing, mut anim_state)) = query.get_single_mut() else {
        return;
    };

    let mut direction = Vec2::ZERO;

    // Gather keyboard input
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    // Update velocity and animation state
    if direction.length_squared() > 0.0 {
        velocity.0 = direction.normalize() * PLAYER_SPEED;
        anim_state.is_moving = true;

        // Update facing direction based on primary axis
        // Priority: vertical movement > horizontal movement
        if direction.y.abs() > direction.x.abs() {
            *facing = if direction.y > 0.0 { Facing::Up } else { Facing::Down };
        } else if direction.x != 0.0 {
            *facing = if direction.x > 0.0 { Facing::Right } else { Facing::Left };
        }
    } else {
        velocity.0 = Vec2::ZERO;
        anim_state.is_moving = false;
        anim_state.current_frame = 1; // Reset to idle frame
    }
}

fn apply_movement(
    time: Res<Time>,
    mut query: Query<(&Velocity, &mut Transform), With<Player>>,
) {
    for (velocity, mut transform) in &mut query {
        transform.translation.x += velocity.0.x * time.delta_secs();
        transform.translation.y += velocity.0.y * time.delta_secs();
    }
}

fn animate_player(
    time: Res<Time>,
    mut query: Query<(&mut AnimationState, &Facing, &mut Sprite), With<Player>>,
) {
    for (mut anim_state, facing, mut sprite) in &mut query {
        if !anim_state.is_moving {
            // Set to idle frame (middle frame of current direction)
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = facing.sprite_row() * 3 + 1;
            }
            continue;
        }

        // Tick animation timer
        anim_state.frame_timer.tick(time.delta());

        if anim_state.frame_timer.just_finished() {
            // Cycle through frames 0, 1, 2
            anim_state.current_frame = (anim_state.current_frame + 1) % 3;

            // Update sprite atlas index
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = facing.sprite_row() * 3 + anim_state.current_frame;
            }
        }
    }
}
```

**Key Design Decisions**:
- **8-directional input**: Captures all 8 directions but displays in 4 cardinal directions
- **Velocity component**: Separates input from movement for future physics integration
- **Facing enum**: Tracks which direction player is looking (for animations/interactions)
- **Animation cycling**: 3 frames per direction (left, middle, right) loops at 0.15s per frame
- **Idle animation**: Stops at middle frame when not moving

### 2. Update main.rs to Include Player

Modify `src/main.rs`:

```rust
use bevy::prelude::*;

mod game_state;
mod player;

use game_state::{GameState, GameStatePlugin};
use player::PlayerPlugin;

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
        .add_plugins((
            GameStatePlugin,
            PlayerPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        .add_systems(Update, test_state_transitions.run_if(in_state(GameState::Playing)))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    info!("SRE Game initialized");
}

fn on_enter_loading(mut next_state: ResMut<NextState<GameState>>) {
    info!("Entering Loading state");
    next_state.set(GameState::Playing);
}

fn on_enter_playing() {
    info!("Entering Playing state - player can explore");
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}

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

### 3. Add Placeholder Sprite Asset

For testing, you need a sprite at `assets/textures/characters/Amy-Walking.png`.

**Option A**: Copy from original game
If available at `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/characters/Amy-Walking.png`:

```bash
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/characters/Amy-Walking.png \
   assets/textures/characters/Amy-Walking.png
```

**Option B**: Create a temporary test sprite
If the original asset isn't available yet, create a placeholder:
- Use any 96x128 pixel sprite sheet (3 columns × 4 rows, 32x32 each)
- Or temporarily use Bevy's built-in texture by loading a simple colored square

**Note**: Verify the actual sprite dimensions by checking the PNG file. If Amy-Walking.png uses different dimensions, update the `UVec2::new(32, 32)` line in spawn_player accordingly.

### 4. Test Player Movement

Run the application:

```bash
cargo run
```

Expected behavior:
- Player sprite (Amy) appears at center of screen
- WASD or Arrow keys move the player in 8 directions
- Player faces the correct cardinal direction
- Walking animation plays when moving
- Player stops at idle frame when keys released
- Press D to enter dialogue mode (player stops moving)
- Press Escape to return to playing mode

### 5. Debug Player Position (Optional)

Add a debug system to verify movement is working:

Add to `src/player.rs`:

```rust
fn debug_player_position(
    query: Query<(&Transform, &Velocity), With<Player>>,
) {
    for (transform, velocity) in &query {
        if velocity.0.length_squared() > 0.0 {
            debug!(
                "Player at ({:.1}, {:.1}), velocity: ({:.1}, {:.1})",
                transform.translation.x,
                transform.translation.y,
                velocity.0.x,
                velocity.0.y
            );
        }
    }
}

// Add to plugin build():
.add_systems(Update, debug_player_position.run_if(in_state(GameState::Playing)))
```

This will log player position when moving (helpful for verifying movement works before camera follows).

## Success Criteria

- [ ] `src/player.rs` created with Player, Velocity, Facing, AnimationState components
- [ ] `PlayerPlugin` integrated into main.rs
- [ ] Player sprite renders on screen at origin
- [ ] WASD/Arrow keys move player in all 8 directions
- [ ] Player sprite animates when moving
- [ ] Player faces correct direction (up/down/left/right)
- [ ] Player stops at idle frame when not moving
- [ ] Movement only active in `GameState::Playing` (not in Dialogue)
- [ ] No compilation errors or warnings

## Sprite Sheet Format Reference

**RPGMaker MZ Character Format**:
```
[Frame 0] [Frame 1] [Frame 2]  ← Row 0: Down
[Frame 0] [Frame 1] [Frame 2]  ← Row 1: Left
[Frame 0] [Frame 1] [Frame 2]  ← Row 2: Right
[Frame 0] [Frame 1] [Frame 2]  ← Row 3: Up
```

- Frame 0: Left foot forward
- Frame 1: Standing (idle)
- Frame 2: Right foot forward

## Known Issues / Future Improvements

- **Collision detection**: Not implemented yet (player can walk through walls)
- **Diagonal sprites**: Player uses 4-directional sprites for 8-directional movement
- **Movement smoothing**: Instant start/stop (could add acceleration later)
- **Animation speed**: May need tuning based on sprite design

## Next Steps

After completing this task:
1. **04-camera-system.md**: Camera will follow the player
2. **05-tilemap-rendering.md**: Add collision boundaries to restrict movement
3. **07-npc-interactions.md**: Use `Facing` direction to determine which NPC to talk to

## Notes for Implementation

- Verify sprite dimensions before implementing - adjust `UVec2::new()` accordingly
- If sprite has padding/margins, use the `padding` and `offset` parameters in `from_grid()`
- Test diagonal movement feels natural (may need to adjust speed multiplier)
- Player should spawn at game origin (0, 0) - tilemap will be centered around this

## Reference Files

- Original sprite: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/characters/Amy-Walking.png`
- Character settings: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Amy-character-settings.json`
