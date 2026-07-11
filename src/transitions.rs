use bevy::prelude::*;
use crate::game_state::{GameState, Scene};
use crate::map_data::{scene_from_str, world_to_tile};
use crate::player::Player;
use crate::tilemap::{CollisionMap, MapExits, PendingArrival};

/// Watches the player's position against the current map's exit triggers and
/// drives scene transitions ("Transfer Player" doors ported from RPGMaker).
pub struct TransitionsPlugin;

impl Plugin for TransitionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, check_map_exits.run_if(in_state(GameState::Playing)));
    }
}

fn check_map_exits(
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    map_exits: Option<Res<MapExits>>,
    collision_map: Option<Res<CollisionMap>>,
    mut next_scene: ResMut<NextState<Scene>>,
) {
    let Some(map_exits) = map_exits else {
        return;
    };
    // We need the current map's dimensions to convert world -> tile space;
    // CollisionMap is inserted/removed by spawn_map/despawn_map on the same
    // lifecycle as MapExits, so it's always present alongside it.
    let Some(collision_map) = collision_map else {
        return;
    };
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let (tile_x, tile_y) = world_to_tile(
        player_transform.translation.truncate(),
        collision_map.width,
        collision_map.height,
    );

    for exit in &map_exits.0 {
        if exit.trigger_x as i32 != tile_x || exit.trigger_y as i32 != tile_y {
            continue;
        }

        let Some(target_scene) = scene_from_str(&exit.target_scene) else {
            error!(
                "Map exit at ({}, {}) references unknown scene '{}' - ignoring",
                exit.trigger_x, exit.trigger_y, exit.target_scene
            );
            continue;
        };

        info!(
            "Player triggered exit at tile ({}, {}) -> {:?} (spawn at {}, {})",
            tile_x, tile_y, target_scene, exit.target_spawn_x, exit.target_spawn_y
        );

        commands.insert_resource(PendingArrival {
            spawn_x: exit.target_spawn_x,
            spawn_y: exit.target_spawn_y,
        });
        next_scene.set(target_scene);
        // Only honor the first matching exit on this tile.
        break;
    }
}
