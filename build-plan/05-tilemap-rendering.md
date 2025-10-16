# Build Plan 05: Tilemap Rendering with bevy_ecs_tilemap

## Objective

Implement tilemap rendering for the game world using `bevy_ecs_tilemap`. Start with Town of Endgame (Map002) as the hub area. This provides the environment for player exploration and NPC placement.

## Context

**Original Game Maps**:
- **Map002 - Town of Endgame**: 34x39 tiles, hub connecting all team areas
- **Map004 - Team Marathon**: 24x21 tiles, interior of Team Marathon building
- Tile size: 48x48 pixels
- Tileset: Visustella Fantasy Tiles MZ (Tileset 12 for Town of Endgame)

**Implementation Strategy**:
- Use `bevy_ecs_tilemap` for efficient tilemap rendering
- Start with a simple test map to validate the system
- Later: Port actual map data from RPGMaker MZ JSON files
- Implement collision layer (walkable vs blocked tiles)

**bevy_ecs_tilemap** provides:
- High-performance tilemap rendering
- Multiple layers (ground, decoration, collision)
- Integration with Bevy's ECS
- Chunk-based rendering for large maps

## Prerequisites

- Completed: **01-project-setup.md** through **04-camera-system.md**
- `bevy_ecs_tilemap = "0.17"` in Cargo.toml
- Player and camera systems functional
- Assets directory structure created

## Tasks

### 1. Create tilemap.rs Module

Create `src/tilemap.rs`:

```rust
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use crate::game_state::{GameState, Scene};
use crate::camera::{MainCamera, CameraFollow, CameraBounds};

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .add_systems(OnEnter(Scene::TownOfEndgame), spawn_town_of_endgame)
            .add_systems(OnExit(Scene::TownOfEndgame), despawn_map);
    }
}

/// Marker for map entities (for cleanup)
#[derive(Component)]
pub struct Map;

/// Tile collision types
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TileCollision {
    Walkable,
    Blocked,
}

/// Resource storing collision data for current map
#[derive(Resource)]
pub struct CollisionMap {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<TileCollision>,
}

impl CollisionMap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            tiles: vec![TileCollision::Walkable; (width * height) as usize],
        }
    }

    pub fn set_tile(&mut self, x: u32, y: u32, collision: TileCollision) {
        if x < self.width && y < self.height {
            let index = (y * self.width + x) as usize;
            self.tiles[index] = collision;
        }
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        let index = (y as u32 * self.width + x as u32) as usize;
        self.tiles.get(index) == Some(&TileCollision::Walkable)
    }
}

fn spawn_town_of_endgame(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
) {
    info!("Spawning Town of Endgame map");

    // Map dimensions (from original Map002.json)
    const MAP_WIDTH: u32 = 34;
    const MAP_HEIGHT: u32 = 39;
    const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 48.0, y: 48.0 };
    const GRID_SIZE: TilemapGridSize = TilemapGridSize { x: 48.0, y: 48.0 };

    // Load tileset texture
    let texture_handle = asset_server.load("textures/tilesets/town_tileset.png");

    // Create tilemap entity
    let map_size = TilemapSize { x: MAP_WIDTH, y: MAP_HEIGHT };
    let tilemap_entity = commands.spawn_empty().id();

    let mut tile_storage = TileStorage::empty(map_size);

    // Create a simple test pattern (will be replaced with actual map data in step 09)
    // For now: grass tiles with some blocked areas
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let tile_pos = TilePos { x, y };

            // Simple pattern: edges are walls, center is walkable
            let texture_index = if x == 0 || y == 0 || x == MAP_WIDTH - 1 || y == MAP_HEIGHT - 1 {
                1 // Wall tile
            } else {
                0 // Grass tile
            };

            let tile_entity = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(texture_index),
                        ..default()
                    },
                    Map,
                ))
                .id();

            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    // Spawn the tilemap with all components
    commands.entity(tilemap_entity).insert((
        TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: TilemapTexture::Single(texture_handle),
            tile_size: TILE_SIZE,
            transform: Transform::from_xyz(
                -(MAP_WIDTH as f32 * TILE_SIZE.x) / 2.0,
                -(MAP_HEIGHT as f32 * TILE_SIZE.y) / 2.0,
                0.0,
            ),
            ..default()
        },
        Map,
    ));

    // Create collision map (will be populated from actual map data in step 09)
    let mut collision_map = CollisionMap::new(MAP_WIDTH, MAP_HEIGHT);

    // For now: mark edges as blocked
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if x == 0 || y == 0 || x == MAP_WIDTH - 1 || y == MAP_HEIGHT - 1 {
                collision_map.set_tile(x, y, TileCollision::Blocked);
            }
        }
    }

    commands.insert_resource(collision_map);

    // Set camera bounds to map dimensions
    if let Ok(mut camera_follow) = camera_query.get_single_mut() {
        let map_width_pixels = MAP_WIDTH as f32 * TILE_SIZE.x;
        let map_height_pixels = MAP_HEIGHT as f32 * TILE_SIZE.y;

        camera_follow.bounds = Some(CameraBounds::from_map_size(
            map_width_pixels,
            map_height_pixels,
            960.0,  // Half of viewport width (1920 / 2)
            540.0,  // Half of viewport height (1080 / 2)
        ));

        info!("Camera bounds set to map size: {}x{} pixels", map_width_pixels, map_height_pixels);
    }
}

fn despawn_map(
    mut commands: Commands,
    map_query: Query<Entity, With<Map>>,
) {
    for entity in &map_query {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<CollisionMap>();
    info!("Map despawned");
}
```

**Key Design Decisions**:
- **Centered origin**: Map is positioned so (0, 0) is at the center
- **Simple test pattern**: Edges are walls, interior is walkable (will be replaced with real data)
- **Collision map resource**: Separate data structure for walkability queries
- **Camera bounds integration**: Automatically sets camera limits based on map size
- **Entity cleanup**: `Map` marker component for despawning on scene change

### 2. Add Collision Detection to Player Movement

Update `src/player.rs` to check collisions before moving:

Add this function to `src/player.rs`:

```rust
use crate::tilemap::CollisionMap;

// Modify the apply_movement system:
fn apply_movement(
    time: Res<Time>,
    collision_map: Option<Res<CollisionMap>>,
    mut query: Query<(&Velocity, &mut Transform), With<Player>>,
) {
    const TILE_SIZE: f32 = 48.0;

    for (velocity, mut transform) in &mut query {
        if velocity.0.length_squared() == 0.0 {
            continue;
        }

        // Calculate new position
        let delta_x = velocity.0.x * time.delta_secs();
        let delta_y = velocity.0.y * time.delta_secs();
        let new_x = transform.translation.x + delta_x;
        let new_y = transform.translation.y + delta_y;

        // Check collision if map exists
        let can_move = if let Some(collision_map) = &collision_map {
            // Convert world position to tile coordinates
            let tile_x = ((new_x / TILE_SIZE) + (collision_map.width as f32 / 2.0)) as i32;
            let tile_y = ((new_y / TILE_SIZE) + (collision_map.height as f32 / 2.0)) as i32;

            collision_map.is_walkable(tile_x, tile_y)
        } else {
            true // No collision map = no restrictions
        };

        if can_move {
            transform.translation.x = new_x;
            transform.translation.y = new_y;
        }
    }
}
```

This prevents the player from walking through walls.

### 3. Update main.rs to Include Tilemap Plugin

Modify `src/main.rs`:

```rust
use bevy::prelude::*;

mod game_state;
mod player;
mod camera;
mod tilemap;

use game_state::{GameState, GameStatePlugin, Scene};
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};
use tilemap::TilemapPlugin;

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
            TilemapPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        .add_systems(Update, test_state_transitions.run_if(in_state(GameState::Playing)))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        CameraFollow::default(),
        Transform::from_xyz(0.0, 0.0, 999.9),
    ));

    info!("SRE Game initialized");
}

fn on_enter_loading(
    mut next_state: ResMut<NextState<GameState>>,
    mut next_scene: ResMut<NextState<Scene>>,
) {
    info!("Entering Loading state");
    next_state.set(GameState::Playing);
    next_scene.set(Scene::TownOfEndgame); // Start in town
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
- Added `tilemap` module and `TilemapPlugin`
- `on_enter_loading` now also sets `Scene::TownOfEndgame`
- This triggers the map spawn system

### 4. Create Temporary Tileset Asset

For testing, create a simple tileset image:

**Option A**: Copy from original game
If Visustella tiles are available:
```bash
# Copy the town tileset (Tileset 12 based on Map002.json)
# Find the correct tileset file in the original game directory
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/tilesets/[TilesetName].png \
   assets/textures/tilesets/town_tileset.png
```

**Option B**: Create a placeholder
Create a simple 96x48 pixel image with:
- Top-left 48x48: Green square (grass tile, index 0)
- Top-right 48x48: Gray square (wall tile, index 1)

Save as `assets/textures/tilesets/town_tileset.png`.

### 5. Test Tilemap Rendering

Run the application:

```bash
cargo run
```

Expected behavior:
- Map renders with grass interior and wall borders
- Player spawns at origin (center of map)
- Player can walk around interior but blocked by edges
- Camera follows player and stops at map boundaries
- Map is centered on screen

### 6. Debug Tilemap (Optional)

Add debug visualization to see tile boundaries:

Add to `src/tilemap.rs`:

```rust
fn debug_render_collision_map(
    collision_map: Res<CollisionMap>,
    mut gizmos: Gizmos,
) {
    const TILE_SIZE: f32 = 48.0;

    for y in 0..collision_map.height {
        for x in 0..collision_map.width {
            if !collision_map.is_walkable(x as i32, y as i32) {
                let world_x = (x as f32 - collision_map.width as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;
                let world_y = (y as f32 - collision_map.height as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;

                gizmos.rect_2d(
                    Vec2::new(world_x, world_y),
                    0.0,
                    Vec2::splat(TILE_SIZE),
                    Color::srgba(1.0, 0.0, 0.0, 0.3),
                );
            }
        }
    }
}

// Add to plugin build():
.add_systems(Update, debug_render_collision_map.run_if(in_state(GameState::Playing)))
```

This draws red overlay boxes on blocked tiles (remove after testing).

## Success Criteria

- [ ] `src/tilemap.rs` created with TilemapPlugin
- [ ] Map renders with test pattern (grass + walls)
- [ ] Player spawns at map center
- [ ] Player cannot walk through wall tiles (edges)
- [ ] Camera bounds prevent showing off-map areas
- [ ] Collision detection works (player blocked by walls)
- [ ] Map despawns when changing scenes
- [ ] No compilation errors or warnings

## Map Data Format Reference

**RPGMaker MZ Map JSON** (for step 09 porting):
```json
{
  "width": 34,
  "height": 39,
  "tilesetId": 12,
  "data": [1, 2, 3, ...],  // Flattened tile indices
  "events": [...]          // NPC positions (handled in step 07)
}
```

The `data` array is `width * height` long, row-major order.

## Known Issues / Future Improvements

- **Test pattern only**: Real map data will be ported in step 09
- **Single layer**: No decoration layer yet (can add later)
- **Static tiles**: No animated tiles (water, torches) yet
- **No doors/transitions**: Scene changes will be added in step 07

## Next Steps

After completing this task:
1. **06-dialogue-system.md**: Add UI for conversations
2. **07-npc-interactions.md**: Spawn NPCs on the map
3. **09-content-port.md**: Replace test pattern with actual Town of Endgame map data

## Map Conversion Helper (For Step 09)

When porting real map data, use this conversion logic:

```rust
// Convert RPGMaker tile index to bevy_ecs_tilemap index
fn rpgmaker_to_bevy_tile_index(rpgmaker_index: u32) -> u32 {
    // RPGMaker uses special encoding for autotiles
    // For Visustella tiles, indices may map 1:1
    // This function will be expanded in step 09
    rpgmaker_index
}

// Parse collision from tileset properties
fn tile_is_walkable(tile_index: u32, tileset_data: &TilesetData) -> bool {
    // Check tileset flags for passability
    // Will be implemented with actual tileset metadata in step 09
    true
}
```

## Notes for Implementation

- Tilemap origin is offset so (0, 0) world position is at map center
- Collision detection converts world coords to tile coords for lookup
- Player collision uses tile center point (could be expanded to use corners)
- Camera bounds account for viewport size to prevent showing void

## Reference Files

- Original map: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Map002.json`
- Tileset data: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Tilesets.json`
- Visustella tiles: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/tilesets/`
- bevy_ecs_tilemap docs: https://docs.rs/bevy_ecs_tilemap/
