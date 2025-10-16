# Build Plan 07: NPC Interaction System

## Objective

Implement NPCs with proximity-based interaction triggers. When the player approaches an NPC and presses the interact key, dialogue begins. This connects the dialogue system to the game world.

## Context

**NPC Interaction Pattern**:
1. Player walks near NPC (within interaction radius)
2. Interaction prompt appears ("Press E to talk")
3. Player presses E → triggers `StartDialogueEvent`
4. Dialogue system takes over (step 06)

**MVP NPCs** (Team Marathon - 5 characters):
- **Nyaanager Evie**: Team manager, teaches healthy culture
- **Hidaslo Xela**: SLO expert, teaches error budgets
- **Seventh Daughter of Nine**: Post-mortem specialist
- **Ocean & Luna**: Pair programming advocates (two NPCs)

## Prerequisites

- Completed: **01-project-setup.md** through **06-dialogue-system.md**
- Dialogue system functional with `StartDialogueEvent`
- Tilemap rendering active
- Player can move around map

## Tasks

### 1. Create npc.rs Module

Create `src/npc.rs`:

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::player::Player;
use crate::dialogue::StartDialogueEvent;

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            check_npc_proximity,
            handle_interaction_input,
        ).chain().run_if(in_state(GameState::Playing)));
    }
}

/// Marker component for NPCs
#[derive(Component)]
pub struct Npc {
    pub name: String,
    pub sprite_facing: NpcFacing,
}

/// Which direction the NPC sprite faces
#[derive(Clone, Copy)]
pub enum NpcFacing {
    Down = 0,
    Left = 1,
    Right = 2,
    Up = 3,
}

/// NPC dialogue configuration
#[derive(Component)]
pub struct NpcDialogue {
    pub speaker: String,
    pub portrait_path: String,
    pub lines: Vec<String>,
}

/// Interaction configuration for proximity detection
#[derive(Component)]
pub struct Interactable {
    pub radius: f32,
    pub prompt: String,
}

impl Default for Interactable {
    fn default() -> Self {
        Self {
            radius: 64.0, // About 1.3 tiles at 48px
            prompt: "Press E to talk".to_string(),
        }
    }
}

/// Marker for NPCs currently in range
#[derive(Component)]
struct InRange;

/// UI marker for interaction prompt
#[derive(Component)]
struct InteractionPrompt;

/// Spawn an NPC entity
pub fn spawn_npc(
    commands: &mut Commands,
    asset_server: &AssetServer,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    position: Vec3,
    sprite_sheet: &str,
    npc_data: Npc,
    dialogue: NpcDialogue,
) -> Entity {
    let texture = asset_server.load(format!("textures/characters/{}", sprite_sheet));

    // Standard RPGMaker format: 3 frames × 4 directions, 32x32 each
    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(32, 32),
        3,
        4,
        None,
        None,
    );
    let atlas_layout = texture_atlas_layouts.add(layout);

    // Calculate sprite index based on facing direction (middle frame of direction row)
    let sprite_index = npc_data.sprite_facing as usize * 3 + 1;

    commands.spawn((
        npc_data,
        dialogue,
        Interactable::default(),
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: sprite_index,
            },
        ),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(2.0)),
    )).id()
}

fn check_npc_proximity(
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    npc_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, Without<InRange>)>,
    in_range_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, With<InRange>)>,
) {
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();

    // Check NPCs not yet in range
    for (entity, npc_transform, interactable) in &npc_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if distance <= interactable.radius {
            commands.entity(entity).insert(InRange);
        }
    }

    // Check NPCs currently in range
    for (entity, npc_transform, interactable) in &in_range_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if distance > interactable.radius {
            commands.entity(entity).remove::<InRange>();
        }
    }
}

fn handle_interaction_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
    npc_query: Query<(&Transform, &NpcDialogue), (With<Npc>, With<InRange>)>,
    mut dialogue_events: EventWriter<StartDialogueEvent>,
    asset_server: Res<AssetServer>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();

    // Find closest NPC in range
    let mut closest_npc: Option<(&NpcDialogue, f32)> = None;

    for (npc_transform, dialogue) in &npc_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if let Some((_, closest_dist)) = closest_npc {
            if distance < closest_dist {
                closest_npc = Some((dialogue, distance));
            }
        } else {
            closest_npc = Some((dialogue, distance));
        }
    }

    // Trigger dialogue with closest NPC
    if let Some((dialogue, _)) = closest_npc {
        let portrait = asset_server.load(&dialogue.portrait_path);

        dialogue_events.send(StartDialogueEvent {
            speaker: dialogue.speaker.clone(),
            portrait: Some(portrait),
            lines: dialogue.lines.clone(),
        });

        info!("Interacting with: {}", dialogue.speaker);
    }
}
```

**Key Design Decisions**:
- **Proximity detection**: Checks distance every frame, adds/removes `InRange` marker
- **E key interaction**: Standard interact key (can add others later)
- **Closest NPC selection**: If multiple NPCs in range, talks to nearest one
- **Static NPCs**: NPCs don't move (MVP), animation can be added later
- **Sprite facing**: NPCs face a fixed direction using sprite sheet rows

### 2. Update tilemap.rs to Spawn NPCs

Modify `src/tilemap.rs` to spawn test NPCs in Town of Endgame:

Add to the top:
```rust
use crate::npc::{spawn_npc, Npc, NpcFacing, NpcDialogue};
```

Add this function:
```rust
fn spawn_test_npcs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    info!("Spawning test NPCs");

    // Nyaanager Evie - Team Marathon manager
    spawn_npc(
        &mut commands,
        &asset_server,
        &mut texture_atlas_layouts,
        Vec3::new(100.0, 100.0, 1.0),
        "Nature.png", // Sprite sheet from original game
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

    // Add more test NPCs here...
}
```

Update the plugin to spawn NPCs:
```rust
impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .add_systems(OnEnter(Scene::TownOfEndgame), (
                spawn_town_of_endgame,
                spawn_test_npcs,
            ).chain())
            .add_systems(OnExit(Scene::TownOfEndgame), despawn_map);
    }
}
```

### 3. Update main.rs to Include NPC Plugin

Modify `src/main.rs`:

```rust
use bevy::prelude::*;

mod game_state;
mod player;
mod camera;
mod tilemap;
mod dialogue;
mod npc;

use game_state::{GameState, GameStatePlugin, Scene};
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};
use tilemap::TilemapPlugin;
use dialogue::DialoguePlugin;
use npc::NpcPlugin;

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
            DialoguePlugin,
            NpcPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
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

fn on_enter_loading(
    mut next_state: ResMut<NextState<GameState>>,
    mut next_scene: ResMut<NextState<Scene>>,
) {
    info!("Entering Loading state");
    next_state.set(GameState::Playing);
    next_scene.set(Scene::TownOfEndgame);
}

fn on_enter_playing() {
    info!("Entering Playing state - player can explore");
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}
```

**Note**: Removed `test_dialogue_trigger` - NPCs now trigger dialogue!

### 4. Add NPC Sprite Assets

Copy NPC sprite sheets from original game:

```bash
# Copy Nature.png for Nyaanager Evie
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/characters/Nature.png \
   assets/textures/characters/Nature.png

# Copy other character sprites as needed
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/characters/People1.png \
   assets/textures/characters/People1.png
```

### 5. Test NPC Interaction

Run the application:

```bash
cargo run
```

Expected behavior:
- Map loads with player at center
- NPC (Nyaanager Evie) appears at (100, 100)
- Walk player towards NPC
- When close enough (within ~64 pixels), `InRange` component added
- Press `E` → dialogue begins with Nyaanager Evie
- Dialogue shows portrait, name, and text
- Advance through conversation
- After dialogue, can interact again

### 6. Add Interaction Prompt UI (Optional)

For better UX, show "Press E to talk" when near an NPC:

Add to `src/npc.rs`:

```rust
fn show_interaction_prompts(
    mut commands: Commands,
    in_range_query: Query<&Interactable, (With<Npc>, With<InRange>, Without<InteractionPrompt>)>,
    prompt_query: Query<Entity, With<InteractionPrompt>>,
    asset_server: Res<AssetServer>,
) {
    // Despawn old prompts
    for entity in &prompt_query {
        commands.entity(entity).despawn_recursive();
    }

    // Show prompt if any NPC in range
    if let Some(interactable) = in_range_query.iter().next() {
        commands.spawn((
            InteractionPrompt,
            Text::new(&interactable.prompt),
            TextFont {
                font: asset_server.load("fonts/dialogue.ttf"),
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                left: Val::Percent(50.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ));
    }
}

// Add to plugin build():
.add_systems(Update, show_interaction_prompts.run_if(in_state(GameState::Playing)))
```

This shows "Press E to talk" at top-center when near an NPC.

## Success Criteria

- [ ] `src/npc.rs` created with NpcPlugin
- [ ] NPCs spawn on map at specified positions
- [ ] NPCs render with correct sprite and facing direction
- [ ] Player can walk near NPC
- [ ] `InRange` marker added/removed based on proximity
- [ ] Press E near NPC → dialogue starts
- [ ] Dialogue shows NPC's portrait and text
- [ ] Can interact with NPC multiple times
- [ ] If multiple NPCs in range, interacts with closest
- [ ] No compilation errors or warnings

## NPC Positioning Reference

**World Coordinates**:
- Origin (0, 0) is at center of map
- X increases to the right
- Y increases upward
- Z-layer 1.0 for NPCs (above ground at 0.0, below player at higher values)

**Tile to World Conversion**:
```rust
fn tile_to_world(tile_x: u32, tile_y: u32, map_width: u32, map_height: u32) -> Vec3 {
    const TILE_SIZE: f32 = 48.0;

    let world_x = (tile_x as f32 - map_width as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;
    let world_y = (tile_y as f32 - map_height as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;

    Vec3::new(world_x, world_y, 1.0)
}
```

This will be used in **09-content-port.md** to place NPCs from map data.

## Known Issues / Future Improvements

- **No interaction indicator**: Consider adding icon above NPC's head when in range
- **No facing player**: NPCs always face same direction (could rotate to face player)
- **Static sprites**: No idle animation (could add later)
- **Single interaction**: Each NPC has one dialogue (could add dialogue trees)
- **No collision with NPCs**: Player can walk through them (could add NPC collision)

## Advanced Features (Optional)

### NPC Idle Animation

Add walking animation to NPCs:

```rust
#[derive(Component)]
struct NpcIdleAnimation {
    timer: Timer,
    frames: [usize; 3],
    current_frame_index: usize,
}

fn animate_npc_idle(
    time: Res<Time>,
    mut query: Query<(&mut NpcIdleAnimation, &mut Sprite), With<Npc>>,
) {
    for (mut anim, mut sprite) in &mut query {
        anim.timer.tick(time.delta());

        if anim.timer.just_finished() {
            anim.current_frame_index = (anim.current_frame_index + 1) % anim.frames.len();

            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = anim.frames[anim.current_frame_index];
            }
        }
    }
}
```

### NPC Faces Player

Rotate NPC sprite to face player when in range:

```rust
fn npc_face_player(
    player_query: Query<&Transform, With<Player>>,
    mut npc_query: Query<(&Transform, &mut Sprite, &mut Npc), With<InRange>>,
) {
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    for (npc_transform, mut sprite, mut npc) in &mut npc_query {
        let to_player = player_transform.translation - npc_transform.translation;

        let new_facing = if to_player.y.abs() > to_player.x.abs() {
            if to_player.y > 0.0 { NpcFacing::Up } else { NpcFacing::Down }
        } else {
            if to_player.x > 0.0 { NpcFacing::Right } else { NpcFacing::Left }
        };

        npc.sprite_facing = new_facing;

        if let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = new_facing as usize * 3 + 1;
        }
    }
}
```

## Next Steps

After completing this task:
1. **08-asset-loading.md**: Proper asset loading for all NPC sprites and portraits
2. **09-content-port.md**: Load NPC positions and dialogue from RPGMaker JSON data
3. Add remaining Team Marathon NPCs (Hidaslo Xela, Seventh Daughter, Ocean, Luna)

## Data Format for Content Port

NPC data in JSON (for step 09):

```json
{
  "npc_id": "nyaanager_evie",
  "position": { "x": 12, "y": 8 },
  "sprite": "Nature.png",
  "facing": "down",
  "dialogue": {
    "speaker": "Nyaanager Evie",
    "portrait": "Nature",
    "portrait_index": 2,
    "lines": [
      "I do my best to protect them but the pressure from Mahogany Row is getting to me.",
      "We had an incident last night but I told the team to take the day off."
    ]
  }
}
```

## Notes for Implementation

- Proximity check runs every frame - could optimize with spatial partitioning later
- `InRange` marker avoids redundant event firing
- Interaction radius should be slightly larger than one tile for comfortable UX
- NPC Z-layer should be consistent (1.0) for proper rendering order
- Sprite facing uses standard RPGMaker row indices (0=down, 1=left, 2=right, 3=up)

## Reference Files

- Original NPC event data: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Map004.json` (Team Marathon)
- Character sprites: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/characters/`
- Portraits: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/faces/`
