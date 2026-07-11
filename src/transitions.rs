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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::map_data::{ExitData, tile_to_world};

    // Town of Endgame's real, converted exits (assets/data/maps/town_of_endgame.json)
    // as of the Team Marathon Retro door fix - kept inline so this test fails
    // loudly if a future re-conversion ever changes the real door's trigger
    // tile or target without the test being updated to match.
    fn town_of_endgame_exits() -> Vec<ExitData> {
        vec![
            ExitData { trigger_x: 8, trigger_y: 29, target_scene: "TeamMarathonRetro".into(), target_spawn_x: 12, target_spawn_y: 15 },
            ExitData { trigger_x: 23, trigger_y: 20, target_scene: "TeamDisco".into(), target_spawn_x: 7, target_spawn_y: 13 },
            ExitData { trigger_x: 6, trigger_y: 18, target_scene: "TeamInferno".into(), target_spawn_x: 11, target_spawn_y: 18 },
            ExitData { trigger_x: 29, trigger_y: 13, target_scene: "MahoganyRow".into(), target_spawn_x: 16, target_spawn_y: 10 },
        ]
    }

    // Town of Endgame's own map dimensions (assets/data/maps/town_of_endgame.json).
    const TOWN_WIDTH: u32 = 34;
    const TOWN_HEIGHT: u32 = 39;

    fn setup_world(player_tile: (u32, u32), exits: Vec<ExitData>, width: u32, height: u32) -> World {
        let mut world = World::new();
        world.init_resource::<NextState<Scene>>();
        world.insert_resource(MapExits(exits));
        world.insert_resource(CollisionMap::new(width, height));

        let world_pos = tile_to_world(player_tile.0, player_tile.1, width, height);
        world.spawn((Player, Transform::from_xyz(world_pos.x, world_pos.y, 1.0)));

        world
    }

    #[test]
    fn player_on_real_town_door_triggers_team_marathon_retro() {
        // The one door in Town of Endgame that's actually themed "Team
        // Marathon" transfers to the Retro map, not the base map - see the
        // NOTE in tools/convert_maps.py. This test pins that behavior so a
        // future change can't silently regress the door back to the wrong
        // (unreachable-in-the-original) destination.
        let mut world = setup_world((8, 29), town_of_endgame_exits(), TOWN_WIDTH, TOWN_HEIGHT);

        world.run_system_once(check_map_exits).unwrap();

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::TeamMarathonRetro)),
            "expected a pending transition to TeamMarathonRetro, got {next:?}"
        );

        let arrival = world.resource::<PendingArrival>();
        assert_eq!((arrival.spawn_x, arrival.spawn_y), (12, 15));
    }

    #[test]
    fn player_off_any_exit_tile_triggers_nothing() {
        let mut world = setup_world((0, 0), town_of_endgame_exits(), TOWN_WIDTH, TOWN_HEIGHT);

        world.run_system_once(check_map_exits).unwrap();

        assert!(matches!(world.resource::<NextState<Scene>>(), NextState::Unchanged));
        assert!(world.get_resource::<PendingArrival>().is_none());
    }

    #[test]
    fn each_town_door_targets_its_real_destination() {
        // Covers all four real doors (not just the Retro one above) so a
        // future edit to the exit table can't silently swap two targets.
        let cases: &[((u32, u32), Scene, (u32, u32))] = &[
            ((8, 29), Scene::TeamMarathonRetro, (12, 15)),
            ((23, 20), Scene::TeamDisco, (7, 13)),
            ((6, 18), Scene::TeamInferno, (11, 18)),
            ((29, 13), Scene::MahoganyRow, (16, 10)),
        ];

        for &(trigger_tile, expected_scene, expected_spawn) in cases {
            let mut world = setup_world(trigger_tile, town_of_endgame_exits(), TOWN_WIDTH, TOWN_HEIGHT);
            world.run_system_once(check_map_exits).unwrap();

            let next = world.resource::<NextState<Scene>>();
            assert!(
                matches!(next, NextState::Pending(scene) if *scene == expected_scene),
                "tile {trigger_tile:?}: expected Pending({expected_scene:?}), got {next:?}"
            );
            let arrival = world.resource::<PendingArrival>();
            assert_eq!((arrival.spawn_x, arrival.spawn_y), expected_spawn, "tile {trigger_tile:?}");
        }
    }

    #[test]
    fn each_team_room_door_returns_to_town_of_endgame() {
        // The single "To Town" door baked into each of these three
        // interior maps (assets/data/maps/team_disco.json,
        // team_inferno.json, mahogany_row.json), completing the round
        // trip that `each_town_door_targets_its_real_destination` only
        // covers in the Town -> room direction. Confirmed unconditional
        // in the original RPGMaker data too: Map005/006/007.json's "To
        // Town" event has exactly one page, with every one of its
        // condition *Valid flags (switch1/switch2/variable/item/actor/
        // selfSwitch) false - so there's no story-gate to encode here,
        // unlike a conditional door would require.
        //
        // Each room's own width/height (not Town's) is required since
        // world_to_tile's tile math is a function of the current map's
        // dimensions.
        let cases: &[(&str, (u32, u32), u32, u32, (u32, u32))] = &[
            ("Team Disco", (7, 14), 15, 19, (23, 21)),
            ("Team Inferno", (11, 19), 23, 24, (6, 19)),
            ("Mahogany Row", (16, 11), 25, 19, (29, 14)),
        ];

        for &(label, trigger_tile, width, height, expected_spawn) in cases {
            let exits = vec![ExitData {
                trigger_x: trigger_tile.0,
                trigger_y: trigger_tile.1,
                target_scene: "TownOfEndgame".into(),
                target_spawn_x: expected_spawn.0,
                target_spawn_y: expected_spawn.1,
            }];
            let mut world = setup_world(trigger_tile, exits, width, height);
            world.run_system_once(check_map_exits).unwrap();

            let next = world.resource::<NextState<Scene>>();
            assert!(
                matches!(next, NextState::Pending(scene) if *scene == Scene::TownOfEndgame),
                "{label} door at {trigger_tile:?}: expected Pending(TownOfEndgame), got {next:?}"
            );
            let arrival = world.resource::<PendingArrival>();
            assert_eq!((arrival.spawn_x, arrival.spawn_y), expected_spawn, "{label} door at {trigger_tile:?}");
        }
    }
}
