use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use crate::game_state::Scene;
use crate::camera::{MainCamera, CameraFollow, CameraBounds};
use crate::npc::{spawn_npc, Npc, NpcDialogue};
use crate::instrumentation::GameTracer;
use crate::assets::GameAssets;
use crate::map_data::{MapData, tile_to_world, facing_from_string};

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ecs_tilemap::TilemapPlugin)
            .add_systems(OnEnter(Scene::TownOfEndgame), spawn_town_of_endgame)
            .add_systems(OnExit(Scene::TownOfEndgame), despawn_map);
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

fn spawn_town_of_endgame(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
    tracer: Option<Res<GameTracer>>,
) {
    info!("Loading Town of Endgame from map data");

    let map = match MapData::load("town_of_endgame") {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to load map: {:?}", e);
            return;
        }
    };

    info!("Loaded map: {} ({}x{})", map.name, map.width, map.height);

    const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 48.0, y: 48.0 };
    const GRID_SIZE: TilemapGridSize = TilemapGridSize { x: 48.0, y: 48.0 };

    let texture_handle = game_assets.town_tileset.clone();
    let map_size = TilemapSize { x: map.width, y: map.height };
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(map_size);

    for y in 0..map.height {
        for x in 0..map.width {
            let tile_pos = TilePos { x, y };
            let index = (y * map.width + x) as usize;

            let texture_index = map.tiles.get(index).copied().unwrap_or(0);

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

    commands.entity(tilemap_entity).insert((
        TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: TilemapTexture::Single(texture_handle),
            tile_size: TILE_SIZE,
            transform: Transform::from_xyz(
                -(map.width as f32 * TILE_SIZE.x) / 2.0,
                -(map.height as f32 * TILE_SIZE.y) / 2.0,
                0.0,
            ),
            ..default()
        },
        Map,
    ));

    let mut collision_map = CollisionMap::new(map.width, map.height);
    for y in 0..map.height {
        for x in 0..map.width {
            if x == 0 || y == 0 || x == map.width - 1 || y == map.height - 1 {
                collision_map.set_tile(x, y, TileCollision::Blocked);
            }
        }
    }
    commands.insert_resource(collision_map);

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

        // Map sprite name to asset handle
        let sprite_handle = match npc_data.sprite.as_str() {
            "Nature" => game_assets.npc_nature.clone(),
            "Mando" => game_assets.npc_mando.clone(),
            "SF_Actor1" => game_assets.npc_sf_actor1.clone(),
            "People1" => game_assets.npc_people1.clone(),
            "Monster" => game_assets.npc_monster.clone(),
            "casey" => game_assets.npc_casey.clone(),
            "Actor1" => game_assets.npc_actor1.clone(),
            "Actor2" => game_assets.npc_actor2.clone(),
            "Evil" => game_assets.npc_evil.clone(),
            "SF_Monster" => game_assets.npc_sf_monster.clone(),
            "People4" => game_assets.npc_people4.clone(),
            _ => {
                warn!("Unknown NPC sprite: {} - skipping {}", npc_data.sprite, npc_data.name);
                continue;
            }
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
            },
            NpcDialogue {
                speaker: npc_data.dialogue.speaker.clone(),
                portrait_path,
                lines: npc_data.dialogue.lines.clone(),
            },
            tracer.as_deref(),
        );

        info!("Spawned NPC: {} at tile ({}, {})", npc_data.name, npc_data.x, npc_data.y);
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
    info!("Map despawned");
}
