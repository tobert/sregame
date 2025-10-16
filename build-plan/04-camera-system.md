# Build Plan 04: 2D Camera System with Player Following

## Objective

Implement a 2D camera that smoothly follows the player character as they move through the game world. The camera provides a stable, centered view for exploration.

## Context

The camera system needs to:
- Keep the player centered on screen during exploration
- Smoothly interpolate movement (no jarring jumps)
- Maintain proper Z-depth for 2D rendering (Z = 999.9)
- Optionally clamp to map boundaries (prevent showing off-map areas)
- Stop following when in Dialogue state (camera stays fixed)

Bevy 0.17 uses `Camera2d` component with simplified spawning (no bundle required).

## Prerequisites

- Completed: **01-project-setup.md**, **02-game-states.md**, **03-player-system.md**
- Player entity spawns and moves in `GameState::Playing`
- Camera2d already spawned in `main.rs` setup

## Tasks

### 1. Create camera.rs Module

Create `src/camera.rs`:

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::player::Player;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            camera_follow_player,
        ).run_if(in_state(GameState::Playing)));
    }
}

/// Marker component for the main game camera
#[derive(Component)]
pub struct MainCamera;

/// Camera configuration for smooth following
#[derive(Component)]
pub struct CameraFollow {
    /// How quickly camera catches up to player (higher = faster, 0-10 typical range)
    pub smoothness: f32,
    /// Optional boundaries to clamp camera position
    pub bounds: Option<CameraBounds>,
}

impl Default for CameraFollow {
    fn default() -> Self {
        Self {
            smoothness: 5.0,
            bounds: None,
        }
    }
}

/// Defines rectangular boundaries for camera movement
#[derive(Clone, Copy)]
pub struct CameraBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl CameraBounds {
    /// Create bounds from map dimensions (in pixels)
    pub fn from_map_size(width: f32, height: f32, camera_half_width: f32, camera_half_height: f32) -> Self {
        Self {
            min_x: -width / 2.0 + camera_half_width,
            max_x: width / 2.0 - camera_half_width,
            min_y: -height / 2.0 + camera_half_height,
            max_y: height / 2.0 - camera_half_height,
        }
    }

    /// Clamp a position to these bounds
    fn clamp(&self, mut position: Vec3) -> Vec3 {
        position.x = position.x.clamp(self.min_x, self.max_x);
        position.y = position.y.clamp(self.min_y, self.max_y);
        position
    }
}

/// Smoothly follows the player with interpolation
fn camera_follow_player(
    mut camera_query: Query<(&mut Transform, &CameraFollow), (With<MainCamera>, Without<Player>)>,
    player_query: Query<&Transform, With<Player>>,
    time: Res<Time>,
) {
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    let Ok((mut camera_transform, follow_config)) = camera_query.get_single_mut() else {
        return;
    };

    // Target position is player's position
    let mut target = player_transform.translation;
    target.z = 999.9; // Keep camera at proper 2D depth

    // Apply bounds clamping if configured
    if let Some(bounds) = follow_config.bounds {
        target = bounds.clamp(target);
    }

    // Smooth interpolation using lerp
    let lerp_factor = follow_config.smoothness * time.delta_secs();
    let lerp_factor = lerp_factor.clamp(0.0, 1.0); // Prevent overshooting

    camera_transform.translation = camera_transform.translation.lerp(target, lerp_factor);

    // Ensure Z stays correct (in case of floating point drift)
    camera_transform.translation.z = 999.9;
}
```

**Key Design Decisions**:
- **Smooth following**: Uses `lerp()` to gradually catch up to player
- **Configurable smoothness**: Higher values = camera follows more tightly
- **Optional bounds**: Prevents showing areas outside the map
- **Z-depth enforcement**: Always keeps camera at 999.9 for proper 2D rendering
- **Query separation**: `Without<Player>` prevents query conflicts

### 2. Update main.rs Camera Spawn

Modify `src/main.rs` to add camera components:

```rust
use bevy::prelude::*;

mod game_state;
mod player;
mod camera;

use game_state::{GameState, GameStatePlugin};
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};

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
            CameraPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        .add_systems(Update, test_state_transitions.run_if(in_state(GameState::Playing)))
        .run();
}

fn setup(mut commands: Commands) {
    // Spawn camera with following behavior
    commands.spawn((
        Camera2d,
        MainCamera,
        CameraFollow::default(),
        Transform::from_xyz(0.0, 0.0, 999.9),
    ));

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

**Changes**:
- Added `camera` module import
- Added `CameraPlugin` to plugin list
- Camera now spawned with `MainCamera`, `CameraFollow`, and initial `Transform`
- Initial position at origin (0, 0, 999.9)

### 3. Test Camera Following

Run the application:

```bash
cargo run
```

Expected behavior:
- Camera starts centered on player at origin
- When player moves with WASD/arrows, camera smoothly follows
- Camera keeps player centered on screen
- Movement feels smooth (no jerky following)
- Press D to enter dialogue mode → camera stops following
- Press Escape to return to playing → camera resumes following

### 4. Tune Smoothness (Optional)

If camera feels too slow or too fast, adjust the smoothness value:

In `src/camera.rs`, modify the `Default` implementation:

```rust
impl Default for CameraFollow {
    fn default() -> Self {
        Self {
            smoothness: 8.0, // Higher = tighter following (try 3.0 - 10.0)
            bounds: None,
        }
    }
}
```

**Smoothness Guidelines**:
- `3.0` = Loose, floaty camera (good for platformers)
- `5.0` = Balanced, comfortable following
- `8.0` = Tight following (good for top-down RPGs)
- `15.0+` = Nearly instant (feels stiff)

### 5. Add Camera Bounds (For Later Use)

The `CameraBounds` system is implemented but not active yet. It will be used in step 05 when maps are loaded.

Example usage (for reference, don't implement yet):

```rust
// When spawning a map in step 05:
fn setup_map(
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
) {
    let Ok(mut camera_follow) = camera_query.get_single_mut() else {
        return;
    };

    // Town of Endgame is 34x39 tiles at 48px each = 1632x1872 pixels
    camera_follow.bounds = Some(CameraBounds::from_map_size(
        1632.0,  // map width in pixels
        1872.0,  // map height in pixels
        960.0,   // half of camera viewport width
        540.0,   // half of camera viewport height
    ));
}
```

This will be integrated in **05-tilemap-rendering.md**.

## Success Criteria

- [ ] `src/camera.rs` created with CameraPlugin, MainCamera, CameraFollow
- [ ] `CameraPlugin` integrated into main.rs
- [ ] Camera smoothly follows player during movement
- [ ] Camera maintains Z-depth at 999.9
- [ ] Camera stops following in Dialogue state
- [ ] Smoothness feels comfortable (adjustable via config)
- [ ] No jittering or stuttering during movement
- [ ] No compilation errors or warnings

## Camera Behavior Reference

```
State: Playing
├─→ Player moves → Camera smoothly interpolates to player position
├─→ Player stops → Camera continues until centered on player
└─→ Player at map edge → Camera stops at boundary (when bounds active)

State: Dialogue
└─→ Camera freezes in place (camera_follow_player doesn't run)
```

## Known Issues / Future Improvements

- **Camera shake**: Not implemented (can add for impact effects later)
- **Zoom levels**: Camera is fixed zoom (could add zoom for different areas)
- **Transition effects**: No fade when changing scenes (could add later)
- **Deadzone**: No deadzone (player always centered) - could add for more freedom

## Next Steps

After completing this task:
1. **05-tilemap-rendering.md**: Will set `CameraBounds` based on map dimensions
2. **06-dialogue-system.md**: Camera freezing during dialogue is already handled
3. Later: Add camera effects (shake, zoom, transitions) as polish

## Advanced Features (Optional)

### Camera Shake Effect

If you want to add camera shake (useful for incidents/alerts in the game):

Add to `src/camera.rs`:

```rust
#[derive(Component)]
pub struct CameraShake {
    pub intensity: f32,
    pub duration: Timer,
}

fn apply_camera_shake(
    mut query: Query<(&mut Transform, &mut CameraShake), With<MainCamera>>,
    time: Res<Time>,
) {
    for (mut transform, mut shake) in &mut query {
        shake.duration.tick(time.delta());

        if !shake.duration.finished() {
            let offset_x = (rand::random::<f32>() - 0.5) * shake.intensity;
            let offset_y = (rand::random::<f32>() - 0.5) * shake.intensity;
            transform.translation.x += offset_x;
            transform.translation.y += offset_y;
        }
    }
}
```

This is not required for MVP but can be added later for dramatic effect.

## Notes for Implementation

- Camera lerp factor is clamped to prevent overshooting on slow frames
- The `Without<Player>` query filter prevents Bevy query conflicts
- Camera continues interpolating even after player stops (smooth deceleration)
- Z-depth is enforced every frame to prevent drift from floating point errors

## Reference

- Bevy 2D camera example: https://github.com/bevyengine/bevy/tree/main/examples/2d
- Original game camera behavior: Fixed camera per room in RPGMaker MZ
