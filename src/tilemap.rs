use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use crate::game_state::Scene;
use crate::camera::{MainCamera, CameraFollow, CameraBounds};
use crate::npc::{spawn_npc, Npc, NpcFacing, NpcDialogue};

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ecs_tilemap::TilemapPlugin)
            .add_systems(OnEnter(Scene::TownOfEndgame), (
                spawn_town_of_endgame,
                spawn_test_npcs,
            ).chain())
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
    asset_server: Res<AssetServer>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
) {
    info!("Spawning Town of Endgame map");

    const MAP_WIDTH: u32 = 34;
    const MAP_HEIGHT: u32 = 39;
    const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 48.0, y: 48.0 };
    const GRID_SIZE: TilemapGridSize = TilemapGridSize { x: 48.0, y: 48.0 };

    let texture_handle = asset_server.load("textures/tilesets/town_tileset.png");

    let map_size = TilemapSize { x: MAP_WIDTH, y: MAP_HEIGHT };
    let tilemap_entity = commands.spawn_empty().id();

    let mut tile_storage = TileStorage::empty(map_size);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let tile_pos = TilePos { x, y };

            let texture_index = if x == 0 || y == 0 || x == MAP_WIDTH - 1 || y == MAP_HEIGHT - 1 {
                1
            } else {
                0
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

    let mut collision_map = CollisionMap::new(MAP_WIDTH, MAP_HEIGHT);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if x == 0 || y == 0 || x == MAP_WIDTH - 1 || y == MAP_HEIGHT - 1 {
                collision_map.set_tile(x, y, TileCollision::Blocked);
            }
        }
    }

    commands.insert_resource(collision_map);

    if let Ok(mut camera_follow) = camera_query.single_mut() {
        let map_width_pixels = MAP_WIDTH as f32 * TILE_SIZE.x;
        let map_height_pixels = MAP_HEIGHT as f32 * TILE_SIZE.y;

        camera_follow.bounds = Some(CameraBounds::from_map_size(
            map_width_pixels,
            map_height_pixels,
            960.0,
            540.0,
        ));

        info!("Camera bounds set to map size: {}x{} pixels", map_width_pixels, map_height_pixels);
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

fn spawn_test_npcs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    info!("Spawning test NPCs");

    spawn_npc(
        &mut commands,
        &asset_server,
        &mut texture_atlas_layouts,
        Vec3::new(100.0, 100.0, 1.0),
        "Nature.png",
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
