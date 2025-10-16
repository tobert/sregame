# Build Plan 08: Asset Loading System

## Objective

Implement a proper asset loading system with a loading screen that waits for all game assets (sprites, tilesets, fonts, portraits) to finish loading before starting gameplay.

## Context

Currently, the Loading state immediately transitions to Playing, which can cause:
- Missing textures (pink/magenta checkerboard)
- Font not loaded (text rendering fails)
- NPCs or player spawning without sprites

Bevy loads assets asynchronously. We need to:
1. Track all required assets
2. Check loading progress
3. Show loading screen
4. Transition to Playing only when complete

**Assets to Load**:
- Player sprite: `Amy-Walking.png`
- NPC sprites: `Nature.png`, `People1.png`, etc.
- Tilesets: `town_tileset.png`, `team_marathon_tileset.png`
- Portraits: `Nature.png`, `Amy.png`, etc.
- Fonts: `dialogue.ttf`

## Prerequisites

- Completed: **01-project-setup.md** through **07-npc-interactions.md**
- All game systems functional but may show missing assets
- Asset files copied to `assets/` directory

## Tasks

### 1. Create assets.rs Module

Create `src/assets.rs`:

```rust
use bevy::prelude::*;
use crate::game_state::GameState;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameAssets>()
            .add_systems(OnEnter(GameState::Loading), start_asset_loading)
            .add_systems(Update, check_asset_loading.run_if(in_state(GameState::Loading)));
    }
}

/// Resource holding handles to all game assets
#[derive(Resource)]
pub struct GameAssets {
    // Player
    pub player_sprite: Handle<Image>,

    // NPCs
    pub npc_nature: Handle<Image>,
    pub npc_people1: Handle<Image>,

    // Tilesets
    pub town_tileset: Handle<Image>,

    // Portraits
    pub portrait_nature: Handle<Image>,
    pub portrait_amy: Handle<Image>,

    // Fonts
    pub dialogue_font: Handle<Font>,

    // Loading state
    pub loaded: bool,
}

impl Default for GameAssets {
    fn default() -> Self {
        Self {
            player_sprite: Handle::default(),
            npc_nature: Handle::default(),
            npc_people1: Handle::default(),
            town_tileset: Handle::default(),
            portrait_nature: Handle::default(),
            portrait_amy: Handle::default(),
            dialogue_font: Handle::default(),
            loaded: false,
        }
    }
}

fn start_asset_loading(
    mut game_assets: ResMut<GameAssets>,
    asset_server: Res<AssetServer>,
) {
    info!("Starting asset loading...");

    // Load player assets
    game_assets.player_sprite = asset_server.load("textures/characters/Amy-Walking.png");

    // Load NPC sprites
    game_assets.npc_nature = asset_server.load("textures/characters/Nature.png");
    game_assets.npc_people1 = asset_server.load("textures/characters/People1.png");

    // Load tilesets
    game_assets.town_tileset = asset_server.load("textures/tilesets/town_tileset.png");

    // Load portraits
    game_assets.portrait_nature = asset_server.load("textures/portraits/Nature.png");
    game_assets.portrait_amy = asset_server.load("textures/portraits/Amy.png");

    // Load fonts
    game_assets.dialogue_font = asset_server.load("fonts/dialogue.ttf");

    game_assets.loaded = false;
}

fn check_asset_loading(
    mut game_assets: ResMut<GameAssets>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if game_assets.loaded {
        return;
    }

    // Check if all assets are loaded
    let all_loaded = asset_server.is_loaded_with_dependencies(&game_assets.player_sprite)
        && asset_server.is_loaded_with_dependencies(&game_assets.npc_nature)
        && asset_server.is_loaded_with_dependencies(&game_assets.npc_people1)
        && asset_server.is_loaded_with_dependencies(&game_assets.town_tileset)
        && asset_server.is_loaded_with_dependencies(&game_assets.portrait_nature)
        && asset_server.is_loaded_with_dependencies(&game_assets.portrait_amy)
        && asset_server.is_loaded_with_dependencies(&game_assets.dialogue_font);

    if all_loaded {
        game_assets.loaded = true;
        info!("All assets loaded successfully!");
        next_state.set(GameState::Playing);
    }
}
```

**Key Design Decisions**:
- **Centralized handles**: All asset handles in one resource
- **Dependency checking**: `is_loaded_with_dependencies()` ensures complete loading
- **State-driven**: Only checks while in Loading state
- **Automatic transition**: Moves to Playing when ready

### 2. Add Loading Screen UI

Add loading screen to provide visual feedback:

Add to `src/assets.rs`:

```rust
#[derive(Component)]
struct LoadingScreen;

fn spawn_loading_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("Spawning loading screen");

    commands.spawn((
        LoadingScreen,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
    ))
    .with_children(|parent| {
        // Title text
        parent.spawn((
            Text::new("The Endgame of SRE"),
            TextFont {
                font: asset_server.load("fonts/dialogue.ttf"),
                font_size: 48.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));

        // Loading message
        parent.spawn((
            Text::new("Loading..."),
            TextFont {
                font: asset_server.load("fonts/dialogue.ttf"),
                font_size: 24.0,
                ..default()
            },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
        ));
    });
}

fn despawn_loading_screen(
    mut commands: Commands,
    loading_screen: Query<Entity, With<LoadingScreen>>,
) {
    for entity in &loading_screen {
        commands.entity(entity).despawn_recursive();
    }
    info!("Loading screen despawned");
}

// Update plugin build():
impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameAssets>()
            .add_systems(OnEnter(GameState::Loading), (
                spawn_loading_screen,
                start_asset_loading,
            ))
            .add_systems(Update, check_asset_loading.run_if(in_state(GameState::Loading)))
            .add_systems(OnExit(GameState::Loading), despawn_loading_screen);
    }
}
```

### 3. Update Existing Systems to Use GameAssets

Modify systems to use centralized asset handles instead of loading directly:

**Update `src/player.rs`**:

```rust
use crate::assets::GameAssets;

fn spawn_player(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let texture = game_assets.player_sprite.clone();

    // Rest of spawn_player implementation...
}
```

**Update `src/tilemap.rs`**:

```rust
use crate::assets::GameAssets;

fn spawn_town_of_endgame(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
) {
    let texture_handle = game_assets.town_tileset.clone();

    // Rest of spawn_town_of_endgame implementation...
}
```

**Update `src/npc.rs`**:

```rust
use crate::assets::GameAssets;

pub fn spawn_npc(
    commands: &mut Commands,
    game_assets: &GameAssets,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    position: Vec3,
    sprite_handle: Handle<Image>,  // Changed from string path
    npc_data: Npc,
    dialogue: NpcDialogue,
) -> Entity {
    let texture = sprite_handle;

    // Rest of spawn_npc implementation...
}

// Update spawn_test_npcs:
fn spawn_test_npcs(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    info!("Spawning test NPCs");

    spawn_npc(
        &mut commands,
        &game_assets,
        &mut texture_atlas_layouts,
        Vec3::new(100.0, 100.0, 1.0),
        game_assets.npc_nature.clone(),  // Use pre-loaded handle
        Npc {
            name: "Nyaanager Evie".to_string(),
            sprite_facing: NpcFacing::Down,
        },
        NpcDialogue {
            speaker: "Nyaanager Evie".to_string(),
            portrait_path: "textures/portraits/Nature.png".to_string(),
            lines: vec![
                "I do my best to protect them but the pressure from Mahogany Row is getting to me.".to_string(),
                "We had an incident last night but I told the team to take the day off.".to_string(),
                "They needed rest more than we needed post-mortem heroics.".to_string(),
            ],
        },
    );
}
```

**Update `src/dialogue.rs`**:

```rust
use crate::assets::GameAssets;

fn spawn_dialogue_ui(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    dialogue_queue: Option<Res<DialogueQueue>>,
) {
    let font = game_assets.dialogue_font.clone();

    // Rest of spawn_dialogue_ui implementation...
}
```

### 4. Update main.rs

Modify `src/main.rs` to include AssetsPlugin and remove immediate transition:

```rust
use bevy::prelude::*;

mod game_state;
mod player;
mod camera;
mod tilemap;
mod dialogue;
mod npc;
mod assets;

use game_state::{GameState, GameStatePlugin, Scene};
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};
use tilemap::TilemapPlugin;
use dialogue::DialoguePlugin;
use npc::NpcPlugin;
use assets::AssetsPlugin;

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
            AssetsPlugin,  // Add before other plugins that use assets
            PlayerPlugin,
            CameraPlugin,
            TilemapPlugin,
            DialoguePlugin,
            NpcPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
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

// Remove on_enter_loading - it's no longer needed

fn on_enter_playing(mut next_scene: ResMut<NextState<Scene>>) {
    info!("Entering Playing state - player can explore");
    next_scene.set(Scene::TownOfEndgame);  // Moved from on_enter_loading
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}
```

### 5. Test Asset Loading

Run the application:

```bash
cargo run
```

Expected behavior:
- Black loading screen appears with "The Endgame of SRE" and "Loading..."
- Console shows "Starting asset loading..."
- After 1-2 seconds (depending on asset size), console shows "All assets loaded successfully!"
- Loading screen disappears
- Game starts in Playing state with all assets visible
- No pink/magenta missing texture squares
- All text renders correctly

### 6. Add Loading Progress Bar (Optional)

For better UX, show loading progress:

Add to `src/assets.rs`:

```rust
#[derive(Component)]
struct LoadingBar;

fn update_loading_progress(
    asset_server: Res<AssetServer>,
    game_assets: Res<GameAssets>,
    mut bar_query: Query<&mut Node, With<LoadingBar>>,
) {
    let handles = vec![
        game_assets.player_sprite.id(),
        game_assets.npc_nature.id(),
        game_assets.npc_people1.id(),
        game_assets.town_tileset.id(),
        game_assets.portrait_nature.id(),
        game_assets.portrait_amy.id(),
        game_assets.dialogue_font.id(),
    ];

    let total = handles.len();
    let loaded = handles.iter()
        .filter(|id| asset_server.is_loaded_with_dependencies(**id))
        .count();

    let progress = loaded as f32 / total as f32;

    if let Ok(mut style) = bar_query.get_single_mut() {
        style.width = Val::Percent(progress * 100.0);
    }
}

// Add progress bar to spawn_loading_screen:
parent.spawn((
    Node {
        width: Val::Percent(0.0),  // Starts at 0%, grows to 100%
        height: Val::Px(10.0),
        ..default()
    },
    BackgroundColor(Color::srgb(0.3, 0.7, 0.3)),
    LoadingBar,
));

// Add to plugin:
.add_systems(Update, (
    check_asset_loading,
    update_loading_progress,
).run_if(in_state(GameState::Loading)))
```

## Success Criteria

- [ ] `src/assets.rs` created with AssetsPlugin and GameAssets resource
- [ ] Loading screen displays on game start
- [ ] All assets load before gameplay begins
- [ ] Console logs "All assets loaded successfully!"
- [ ] No pink/magenta missing textures in game
- [ ] Player sprite renders correctly
- [ ] NPCs render correctly
- [ ] Tilemap renders correctly
- [ ] Font loads and text displays properly
- [ ] Loading screen disappears after assets loaded
- [ ] No compilation errors or warnings

## Asset Loading Best Practices

**Handle Management**:
- Store handles in Resource to prevent premature unloading
- Clone handles when passing to spawn functions
- Never drop handles of currently-used assets

**Loading Checks**:
- Use `is_loaded_with_dependencies()` not just `is_loaded()`
- Check ALL required assets before transitioning
- Handle loading failures gracefully (add error checking later)

**Performance**:
- Load assets in Loading state, not during gameplay
- Consider asset groups for scene-specific assets
- Use compressed formats (PNG for sprites, compressed fonts)

## Known Issues / Future Improvements

- **No error handling**: If asset fails to load, hangs forever in Loading state
- **No retry mechanism**: Failed loads don't retry
- **Static asset list**: Adding new assets requires code changes
- **No streaming**: All assets loaded upfront (could load per-scene later)

## Advanced Features (Optional)

### Asset Loading from JSON Manifest

Create `assets/manifest.json`:
```json
{
  "player": {
    "sprite": "textures/characters/Amy-Walking.png"
  },
  "npcs": [
    {"id": "nature", "sprite": "textures/characters/Nature.png"},
    {"id": "people1", "sprite": "textures/characters/People1.png"}
  ],
  "tilesets": [
    {"id": "town", "path": "textures/tilesets/town_tileset.png"}
  ]
}
```

Load dynamically instead of hardcoding paths.

### Error Handling for Missing Assets

```rust
fn check_asset_loading(
    // ... existing parameters ...
    mut error_shown: Local<bool>,
) {
    // Check for load errors
    if let LoadState::Failed = asset_server.load_state(&game_assets.player_sprite) {
        if !*error_shown {
            error!("Failed to load player sprite!");
            *error_shown = true;
        }
        return;
    }

    // Rest of loading check...
}
```

## Next Steps

After completing this task:
1. **09-content-port.md**: Port actual map data and dialogue from RPGMaker JSON
2. Add more assets as needed (Team Marathon tileset, additional NPCs)
3. Consider scene-specific asset loading for better performance

## Notes for Implementation

- `is_loaded_with_dependencies()` checks transitive dependencies (e.g., font subsetting)
- Loading usually takes 1-2 seconds on modern hardware
- Asset handles are reference-counted - don't need manual cleanup
- Loading screen font might not load in time - use default or spawn text delayed

## Reference

- Bevy Asset Loading: https://bevyengine.org/learn/book/getting-started/assets/
- Asset Server API: https://docs.rs/bevy/latest/bevy/asset/struct.AssetServer.html
