use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use crate::game_state::Scene;
use crate::camera::{MainCamera, CameraFollow, CameraBounds};
use crate::npc::{spawn_npc, Npc, NpcDialogue};
use crate::instrumentation::GameTracer;
use crate::assets::GameAssets;
use crate::map_data::{MapData, ExitData, tile_to_world, facing_from_string};
use crate::player::Player;

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ecs_tilemap::TilemapPlugin)
            .add_systems(OnEnter(Scene::TownOfEndgame), spawn_map)
            .add_systems(OnEnter(Scene::TeamMarathon), spawn_map)
            .add_systems(OnEnter(Scene::TeamMarathonRetro), spawn_map)
            .add_systems(OnEnter(Scene::TeamDisco), spawn_map)
            .add_systems(OnEnter(Scene::TeamInferno), spawn_map)
            .add_systems(OnEnter(Scene::MahoganyRow), spawn_map)
            .add_systems(OnEnter(Scene::Intro), spawn_map)
            .add_systems(OnEnter(Scene::End), spawn_map)
            .add_systems(OnExit(Scene::TownOfEndgame), despawn_map)
            .add_systems(OnExit(Scene::TeamMarathon), despawn_map)
            .add_systems(OnExit(Scene::TeamMarathonRetro), despawn_map)
            .add_systems(OnExit(Scene::TeamDisco), despawn_map)
            .add_systems(OnExit(Scene::TeamInferno), despawn_map)
            .add_systems(OnExit(Scene::MahoganyRow), despawn_map)
            .add_systems(OnExit(Scene::Intro), despawn_map)
            .add_systems(OnExit(Scene::End), despawn_map);
    }
}

#[derive(Component)]
pub struct Map;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TileCollision {
    Walkable,
    Blocked,
}

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

/// Exit (portal) triggers for the currently loaded map. Same resource
/// lifecycle as `CollisionMap`: inserted by `spawn_map`, removed by
/// `despawn_map`.
#[derive(Resource)]
pub struct MapExits(pub Vec<ExitData>);

/// Set by the transition system just before switching scenes; consumed by
/// `spawn_map` on the following `OnEnter` to place the player at the correct
/// tile in the newly-loaded map. Absent on the very first scene load, since
/// there's no incoming transition then.
#[derive(Resource)]
pub struct PendingArrival {
    pub spawn_x: u32,
    pub spawn_y: u32,
}

/// Per-scene map file + tileset lookup. Tileset keys are a contract with
/// `assets::GameAssets::tilesets` (populated by scanning
/// `assets/textures/tilesets/*.png`): "town_tileset" for the outdoor Town of
/// Endgame map, "inside_tileset" for all interior maps.
pub struct SceneConfig {
    pub map_file: &'static str,
    pub tileset_key: &'static str,
}

pub fn scene_config(scene: Scene) -> SceneConfig {
    match scene {
        Scene::TownOfEndgame => SceneConfig { map_file: "town_of_endgame", tileset_key: "town_tileset" },
        Scene::TeamMarathon => SceneConfig { map_file: "team_marathon", tileset_key: "inside_tileset" },
        Scene::TeamMarathonRetro => SceneConfig { map_file: "team_marathon_retro", tileset_key: "inside_tileset" },
        Scene::TeamDisco => SceneConfig { map_file: "team_disco", tileset_key: "inside_tileset" },
        Scene::TeamInferno => SceneConfig { map_file: "team_inferno", tileset_key: "inside_tileset" },
        Scene::MahoganyRow => SceneConfig { map_file: "mahogany_row", tileset_key: "inside_tileset" },
        Scene::Intro => SceneConfig { map_file: "intro", tileset_key: "inside_tileset" },
        Scene::End => SceneConfig { map_file: "end", tileset_key: "inside_tileset" },
    }
}

fn spawn_map(
    mut commands: Commands,
    scene: Res<State<Scene>>,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
    mut player_query: Query<&mut Transform, With<Player>>,
    pending_arrival: Option<Res<PendingArrival>>,
    tracer: Option<Res<GameTracer>>,
) {
    let config = scene_config(*scene.get());

    info!("Loading {:?} from map data ({})", scene.get(), config.map_file);

    let map = match MapData::load(config.map_file) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to load map '{}': {:?}", config.map_file, e);
            // Don't leave a stale PendingArrival around for some later,
            // unrelated scene load to accidentally consume - a portal that
            // led nowhere shouldn't silently misplace the player next time
            // a map *does* load successfully.
            if pending_arrival.is_some() {
                warn!("Discarding PendingArrival - target scene's map failed to load");
                commands.remove_resource::<PendingArrival>();
            }
            return;
        }
    };

    info!("Loaded map: {} ({}x{})", map.name, map.width, map.height);

    // A missing tileset is a visual gap, not a logical one: the map's
    // collision, exits and NPCs must still come up so the transition system
    // works even for scenes whose art hasn't been authored yet (several
    // interior scenes don't have clean map JSON *or* art yet - see
    // scene_config). Fall back to an empty texture handle and keep going.
    let texture_handle = match game_assets.tilesets.get(config.tileset_key).cloned() {
        Some(handle) => handle,
        None => {
            warn!(
                "Missing tileset '{}' for scene {:?} - rendering without tile art",
                config.tileset_key, scene.get()
            );
            Handle::default()
        }
    };

    const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 48.0, y: 48.0 };
    const GRID_SIZE: TilemapGridSize = TilemapGridSize { x: 48.0, y: 48.0 };
    // Ground and upper layers share one atlas (see tools/convert_maps.py),
    // so both TilemapBundles below reference the same texture handle.
    // Upper renders above the player/NPCs (z=1.0, see player.rs/npc.rs).
    const GROUND_Z: f32 = 0.0;
    const UPPER_Z: f32 = 2.0;

    let map_size = TilemapSize { x: map.width, y: map.height };
    let map_transform_xy = (
        -(map.width as f32 * TILE_SIZE.x) / 2.0,
        -(map.height as f32 * TILE_SIZE.y) / 2.0,
    );

    let ground_entity = commands.spawn_empty().id();
    let mut ground_storage = TileStorage::empty(map_size);

    let upper_entity = commands.spawn_empty().id();
    let mut upper_storage = TileStorage::empty(map_size);

    for y in 0..map.height {
        for x in 0..map.width {
            // Map JSON rows are RPGMaker-ordered (row 0 = top), while
            // bevy_ecs_tilemap's TilePos y=0 is the BOTTOM row, so the row
            // must be flipped here or the whole map renders vertically
            // mirrored. Same convention boundary as map_data::tile_to_world.
            let tile_pos = TilePos { x, y: map.height - 1 - y };
            let index = (y * map.width + x) as usize;

            let ground_index = map.tiles.get(index).copied().unwrap_or(0);
            let ground_tile = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(ground_entity),
                        texture_index: TileTextureIndex(ground_index),
                        ..default()
                    },
                    Map,
                ))
                .id();
            ground_storage.set(&tile_pos, ground_tile);

            let upper_index = map.upper_tiles.get(index).copied().unwrap_or(0);
            let upper_tile = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(upper_entity),
                        texture_index: TileTextureIndex(upper_index),
                        ..default()
                    },
                    Map,
                ))
                .id();
            upper_storage.set(&tile_pos, upper_tile);
        }
    }

    commands.entity(ground_entity).insert((
        TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: ground_storage,
            texture: TilemapTexture::Single(texture_handle.clone()),
            tile_size: TILE_SIZE,
            transform: Transform::from_xyz(map_transform_xy.0, map_transform_xy.1, GROUND_Z),
            ..default()
        },
        Map,
    ));

    commands.entity(upper_entity).insert((
        TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: upper_storage,
            texture: TilemapTexture::Single(texture_handle),
            tile_size: TILE_SIZE,
            transform: Transform::from_xyz(map_transform_xy.0, map_transform_xy.1, UPPER_Z),
            ..default()
        },
        Map,
    ));

    // CollisionMap stays in RPGMaker orientation (y=0 = top row, same as the
    // JSON), because every lookup goes through world_to_tile, which returns
    // RPGMaker-orientation coordinates.
    let mut collision_map = CollisionMap::new(map.width, map.height);
    for y in 0..map.height {
        for x in 0..map.width {
            let index = (y * map.width + x) as usize;
            if map.collision.get(index).copied().unwrap_or(true) {
                collision_map.set_tile(x, y, TileCollision::Blocked);
            }
        }
    }
    commands.insert_resource(collision_map);
    commands.insert_resource(MapExits(map.exits.clone()));

    if let Ok(mut camera_follow) = camera_query.single_mut() {
        let map_width_pixels = map.width as f32 * TILE_SIZE.x;
        let map_height_pixels = map.height as f32 * TILE_SIZE.y;

        camera_follow.bounds = Some(CameraBounds::from_map_size(
            map_width_pixels,
            map_height_pixels,
            960.0,
            540.0,
        ));
    }

    // Spawn NPCs from map data
    info!("Spawning {} NPCs from map data", map.npcs.len());
    for npc_data in &map.npcs {
        let world_pos = tile_to_world(npc_data.x, npc_data.y, map.width, map.height);

        // Map sprite name to asset handle, looked up by filename stem from
        // the data-driven GameAssets::npc_sprites map.
        let Some(sprite_handle) = game_assets.npc_sprites.get(&npc_data.sprite).cloned() else {
            warn!("Unknown NPC sprite: {} - skipping {}", npc_data.sprite, npc_data.name);
            continue;
        };

        let portrait_path = if !npc_data.dialogue.portrait.is_empty() {
            format!("textures/portraits/{}.png", npc_data.dialogue.portrait)
        } else {
            String::new()
        };

        spawn_npc(
            &mut commands,
            &game_assets,
            &mut texture_atlas_layouts,
            Vec3::new(world_pos.x, world_pos.y, 1.0),
            sprite_handle,
            Npc {
                name: npc_data.name.clone(),
                sprite_facing: facing_from_string(&npc_data.facing),
                sprite_slot: npc_data.sprite_index,
            },
            NpcDialogue {
                speaker: npc_data.dialogue.speaker.clone(),
                portrait_path,
                portrait_face_index: npc_data.dialogue.face_index,
                lines: npc_data.dialogue.lines.clone(),
            },
            tracer.as_deref(),
        );

        info!("Spawned NPC: {} at tile ({}, {})", npc_data.name, npc_data.x, npc_data.y);
    }

    // If we arrived via a portal (see transitions.rs), place the player at
    // the target spawn tile. If absent, this is either the very first scene
    // load or a scene the player didn't reach via a portal - leave the
    // player wherever it already is.
    if let Some(arrival) = pending_arrival {
        if let Ok(mut player_transform) = player_query.single_mut() {
            let spawn_pos = tile_to_world(arrival.spawn_x, arrival.spawn_y, map.width, map.height);
            player_transform.translation.x = spawn_pos.x;
            player_transform.translation.y = spawn_pos.y;
            info!("Placed player at incoming spawn tile ({}, {})", arrival.spawn_x, arrival.spawn_y);
        }
        commands.remove_resource::<PendingArrival>();
    }
}

fn despawn_map(
    mut commands: Commands,
    map_query: Query<Entity, With<Map>>,
) {
    for entity in &map_query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<CollisionMap>();
    commands.remove_resource::<MapExits>();
    info!("Map despawned");
}
