use bevy::prelude::*;
use crate::game_state::{Mode, Scene};
use crate::map_data::{scene_from_str, world_to_tile, ExitTrigger};
use crate::player::Player;
use crate::tilemap::{CollisionMap, MapExits, PendingArrival};

/// Watches the player's position against the current map's exit triggers and
/// drives scene transitions ("Transfer Player" doors ported from RPGMaker).
pub struct TransitionsPlugin;

impl Plugin for TransitionsPlugin {
    fn build(&self, app: &mut App) {
        // Gated on Mode::Exploring (rather than GameState::Playing) so a
        // portal can't fire while a dialogue box is showing - Mode only
        // exists at all while GameState::Playing, so this also implies that.
        app.add_systems(Update, (
            check_map_exits,
            animate_door_departure,
        ).chain()
            // After the player has actually moved this frame: exits read
            // the post-move position and this frame's bump messages, not
            // last frame's (kaibo review 2026-07-12, both reviewers).
            .after(crate::player::PlayerMovementSet)
            .run_if(in_state(Mode::Exploring)))
            // Fires the deferred transfer once the scripted scene closes
            // (Mode returns to Exploring). Also runs at game start and
            // after every ordinary dialogue - gated on the resource.
            .add_systems(OnEnter(Mode::Exploring), fire_transfer_after_dialogue);
    }
}

/// A transfer waiting for its scripted scene to finish (the exit had
/// dialogue segments). Inserted by check_map_exits when the exit fires;
/// consumed when Mode re-enters Exploring, i.e. when the dialogue closes -
/// including an Escape skip, which still transfers, matching "skip scene"
/// semantics.
#[derive(Resource)]
pub struct PendingTransferAfterDialogue {
    pub(crate) target_scene: Scene,
    pub(crate) spawn_x: u32,
    pub(crate) spawn_y: u32,
    /// True for consent prompts (the End fairies): Escape drops this
    /// resource (see game_state::handle_escape_key) so the transfer never
    /// fires. False for scripted scenes like the retrospective, where
    /// Escape skips the scene but the transfer still happens.
    pub cancel_on_escape: bool,
}

fn fire_transfer_after_dialogue(
    mut commands: Commands,
    pending: Option<Res<PendingTransferAfterDialogue>>,
    mut next_scene: ResMut<NextState<Scene>>,
) {
    let Some(pending) = pending else {
        return;
    };
    info!("Scripted scene finished - transferring to {:?}", pending.target_scene);
    commands.insert_resource(PendingArrival {
        spawn_x: pending.spawn_x,
        spawn_y: pending.spawn_y,
    });
    next_scene.set(pending.target_scene);
    commands.remove_resource::<PendingTransferAfterDialogue>();
}

/// A visible door sprite sitting on an exit trigger tile (the town's
/// `!doors` events - see MapData::doors). Spawned by tilemap::spawn_map,
/// despawned with the map.
#[derive(Component)]
pub struct Door {
    pub tile_x: u32,
    pub tile_y: u32,
    pub sprite_slot: u32,
    pub pattern: u32,
}

/// Present while a door-open sequence runs; the scene transition fires when
/// it finishes. Mirrors the original's Common Event 24 "Open Door": the
/// door's animation row steps closed -> ajar -> half -> open (RPGMaker
/// abuses the 4 direction rows as opening stages), ~3 frames apart, then
/// the transfer happens. Player input and exit checks pause while this
/// resource exists (see player.rs / check_map_exits).
#[derive(Resource)]
pub struct DepartingDoor {
    door: Entity,
    target_scene: Scene,
    spawn_x: u32,
    spawn_y: u32,
    /// 0 = closed (resting); 1..=3 = opening rows; past 3 = transfer.
    stage: u8,
    timer: Timer,
}

/// Original timing: 3 frames at 60fps between row changes.
const DOOR_STAGE_SECONDS: f32 = 0.05;
/// Hold the fully-open door briefly before transferring, standing in for
/// the original's screen fade.
const DOOR_OPEN_HOLD_SECONDS: f32 = 0.2;

fn animate_door_departure(
    mut commands: Commands,
    departing: Option<ResMut<DepartingDoor>>,
    time: Res<Time>,
    mut doors: Query<(&Door, &mut Sprite)>,
    mut next_scene: ResMut<NextState<Scene>>,
) {
    let Some(mut dep) = departing else {
        return;
    };

    dep.timer.tick(time.delta());
    if !dep.timer.just_finished() {
        return;
    }

    dep.stage += 1;

    if dep.stage <= 3 {
        if let Ok((door, mut sprite)) = doors.get_mut(dep.door) {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = crate::character_sheet::atlas_index(
                    door.sprite_slot,
                    u32::from(dep.stage),
                    door.pattern,
                ) as usize;
            }
        }
        let next_wait = if dep.stage == 3 { DOOR_OPEN_HOLD_SECONDS } else { DOOR_STAGE_SECONDS };
        dep.timer = Timer::from_seconds(next_wait, TimerMode::Once);
    } else {
        info!("Door fully open - transferring to {:?}", dep.target_scene);
        commands.insert_resource(PendingArrival {
            spawn_x: dep.spawn_x,
            spawn_y: dep.spawn_y,
        });
        next_scene.set(dep.target_scene);
        // Deliberately NOT removed here: the state transition applies at
        // the end of the frame, so removing now would unfreeze player
        // input (and re-arm check_map_exits) for one frame mid-transfer
        // (kaibo review 2026-07-12). despawn_map clears it when the old
        // scene tears down.
    }
}

/// Converts an exit's dialogue data into the runtime segment form.
fn dialogue_segments(
    dialogue: &[crate::map_data::DialogueSegmentData],
) -> Vec<crate::dialogue::DialogueSegment> {
    dialogue
        .iter()
        .map(|seg| crate::dialogue::DialogueSegment {
            speaker: seg.speaker.clone(),
            portrait_path: if seg.portrait.is_empty() {
                String::new()
            } else {
                format!("textures/portraits/{}.png", seg.portrait)
            },
            portrait_face_index: seg.face_index,
            text: seg.text.clone(),
        })
        .collect()
}

fn check_map_exits(
    mut commands: Commands,
    player_query: Query<(&Transform, &crate::player::Facing), With<Player>>,
    map_exits: Option<Res<MapExits>>,
    collision_map: Option<Res<CollisionMap>>,
    doors: Query<(Entity, &Door)>,
    departing: Option<Res<DepartingDoor>>,
    mut bumps: MessageReader<crate::player::BumpedIntoTile>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dialogue_events: MessageWriter<crate::dialogue::StartDialogueEvent>,
    mut next_scene: ResMut<NextState<Scene>>,
) {
    // A departure is already in flight - don't re-trigger. Still drain the
    // bump messages so stale bumps can't fire an exit later.
    if departing.is_some() {
        bumps.clear();
        return;
    }
    let Some(map_exits) = map_exits else {
        return;
    };
    // We need the current map's dimensions to convert world -> tile space;
    // CollisionMap is inserted/removed by spawn_map/despawn_map on the same
    // lifecycle as MapExits, so it's always present alongside it.
    let Some(collision_map) = collision_map else {
        return;
    };
    let Ok((player_transform, facing)) = player_query.single() else {
        return;
    };

    let (tile_x, tile_y) = world_to_tile(
        crate::player::logical_position(player_transform.translation.truncate()),
        collision_map.width,
        collision_map.height,
    );

    // The tile the player is looking at: action exits also fire when FACED
    // from one tile away (RPGMaker's checkEventTriggerThere), not only when
    // stood on. Amy's playtest: walking up to the retro table and pressing
    // E while facing the parchment did nothing until she happened to stand
    // on the trigger itself.
    let (face_dx, face_dy) = facing.tile_delta();
    let (faced_x, faced_y) = (tile_x + face_dx, tile_y + face_dy);

    // A touch exit fires when the player stands on its tile (walkable exit
    // mats: the interior "To Town" tiles) OR bumps into it (RPGMaker
    // Player-Touch on impassable door tiles - the town doors are
    // collision-blocked, so standing on them is impossible). An ACTION
    // exit only fires when the player stands on it and presses E - the
    // inn's "retro dialog" event is one, and treating it as touch warped
    // players to the End scene for walking near the table.
    let mut touched_tiles: Vec<(i32, i32)> = vec![(tile_x, tile_y)];
    for bump in bumps.read() {
        touched_tiles.push((bump.tile_x, bump.tile_y));
    }

    for exit in &map_exits.0 {
        let hit = match exit.trigger {
            ExitTrigger::Touch => touched_tiles
                .iter()
                .any(|&(cx, cy)| exit.trigger_x as i32 == cx && exit.trigger_y as i32 == cy),
            ExitTrigger::Action => {
                keyboard.just_pressed(KeyCode::KeyE)
                    && ((exit.trigger_x as i32 == tile_x && exit.trigger_y as i32 == tile_y)
                        || (exit.trigger_x as i32 == faced_x && exit.trigger_y as i32 == faced_y))
            }
        };
        if !hit {
            continue;
        }

        // A scripted scene with NO destination ("scene-only": the
        // retrospective at the retro table) just plays where the player
        // stands - no transfer, no scene lookup. Leaving the room is its
        // own action (the inn stairs).
        if !exit.dialogue.is_empty() && exit.target_scene.is_empty() {
            info!(
                "Player triggered scene at tile ({}, {}) - no transfer",
                exit.trigger_x, exit.trigger_y
            );
            dialogue_events.write(crate::dialogue::StartDialogueEvent {
                segments: dialogue_segments(&exit.dialogue),
            });
            break;
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
            exit.trigger_x, exit.trigger_y, target_scene, exit.target_spawn_x, exit.target_spawn_y
        );

        // Precedence: a scripted scene plays first (transfer fires when it
        // closes); a door on the tile animates open first; bare exits
        // transfer immediately.
        let door_here = doors.iter().find(|(_, door)| {
            door.tile_x == exit.trigger_x && door.tile_y == exit.trigger_y
        });

        if !exit.dialogue.is_empty() {
            dialogue_events.write(crate::dialogue::StartDialogueEvent {
                segments: dialogue_segments(&exit.dialogue),
            });
            commands.insert_resource(PendingTransferAfterDialogue {
                target_scene,
                spawn_x: exit.target_spawn_x,
                spawn_y: exit.target_spawn_y,
                cancel_on_escape: exit.cancel_on_escape,
            });
        } else if let Some((door_entity, _)) = door_here {
            commands.insert_resource(DepartingDoor {
                door: door_entity,
                target_scene,
                spawn_x: exit.target_spawn_x,
                spawn_y: exit.target_spawn_y,
                stage: 0,
                timer: Timer::from_seconds(DOOR_STAGE_SECONDS, TimerMode::Once),
            });
        } else {
            commands.insert_resource(PendingArrival {
                spawn_x: exit.target_spawn_x,
                spawn_y: exit.target_spawn_y,
            });
            next_scene.set(target_scene);
        }
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
            ExitData { trigger_x: 8, trigger_y: 29, target_scene: "TeamMarathonRetro".into(), target_spawn_x: 12, target_spawn_y: 15, trigger: ExitTrigger::Touch, dialogue: vec![], cancel_on_escape: false },
            ExitData { trigger_x: 23, trigger_y: 20, target_scene: "TeamDisco".into(), target_spawn_x: 7, target_spawn_y: 13, trigger: ExitTrigger::Touch, dialogue: vec![], cancel_on_escape: false },
            ExitData { trigger_x: 6, trigger_y: 18, target_scene: "TeamInferno".into(), target_spawn_x: 11, target_spawn_y: 18, trigger: ExitTrigger::Touch, dialogue: vec![], cancel_on_escape: false },
            ExitData { trigger_x: 29, trigger_y: 13, target_scene: "MahoganyRow".into(), target_spawn_x: 16, target_spawn_y: 10, trigger: ExitTrigger::Touch, dialogue: vec![], cancel_on_escape: false },
        ]
    }

    // Town of Endgame's own map dimensions (assets/data/maps/town_of_endgame.json).
    const TOWN_WIDTH: u32 = 34;
    const TOWN_HEIGHT: u32 = 39;

    fn setup_world(player_tile: (u32, u32), exits: Vec<ExitData>, width: u32, height: u32) -> World {
        let mut world = World::new();
        world.init_resource::<NextState<Scene>>();
        world.init_resource::<Messages<crate::player::BumpedIntoTile>>();
        world.init_resource::<Messages<crate::dialogue::StartDialogueEvent>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.insert_resource(MapExits(exits));
        world.insert_resource(CollisionMap::new(width, height));

        let world_pos = tile_to_world(player_tile.0, player_tile.1, width, height);
        world.spawn((Player, crate::player::Facing::Up, Transform::from_xyz(world_pos.x, world_pos.y, 1.0)));

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

    // The inn's real action exit (assets/data/maps/team_marathon_retro.json):
    // the "retro dialog" event by the table. Regression tests for the warp
    // mine where walking near the table teleported the player to End.
    fn retro_action_exit() -> Vec<ExitData> {
        vec![
            ExitData { trigger_x: 12, trigger_y: 12, target_scene: "End".into(), target_spawn_x: 8, target_spawn_y: 5, trigger: ExitTrigger::Action, dialogue: vec![], cancel_on_escape: false },
        ]
    }

    /// Amy's repro: "walking all the way to the table does not work, but
    /// bumping back just a touch does." Pressed flush against a blocked row
    /// from the south, collision lets the SPRITE center penetrate up to 8px
    /// into that row (feet-anchored box, head-overlaps-wall perspective) -
    /// so a sprite-center tile lookup reported the player as standing IN
    /// the table row and the action exit under their feet stopped matching.
    /// The logical tile must come from the collision box center.
    #[test]
    fn action_exit_fires_when_pressed_flush_against_the_blocked_row() {
        let mut world = setup_world((12, 12), retro_action_exit(), 24, 21);
        // Reproduce the flush pose: sprite center 8px INSIDE row 11
        // (one tile north of the trigger row), exactly where movement
        // stops when walking up into the table.
        let flush_y = tile_to_world(12, 11, 24, 21).y - 16.0;
        let player = world
            .query_filtered::<Entity, With<Player>>()
            .single(&world)
            .unwrap();
        world.get_mut::<Transform>(player).unwrap().translation.y = flush_y;
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);

        world.run_system_once(check_map_exits).unwrap();

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::End)),
            "E flush against the table must still fire the exit, got {next:?}"
        );
    }

    /// A scripted scene with no destination (the retrospective, since Amy
    /// decided the player should stay at the table): the dialogue plays and
    /// NOTHING is transferred, immediately or deferred.
    #[test]
    fn scene_only_exit_plays_dialogue_without_transferring() {
        let mut exits = retro_action_exit();
        exits[0].target_scene = String::new();
        exits[0].dialogue = vec![crate::map_data::DialogueSegmentData {
            speaker: "Nyaanager Evie".into(),
            portrait: "Nature".into(),
            face_index: 4,
            text: "Thanks for helping us with this incident Amy.".into(),
        }];
        let mut world = setup_world((12, 12), exits, 24, 21);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);

        world.run_system_once(check_map_exits).unwrap();

        let sent = world
            .resource::<Messages<crate::dialogue::StartDialogueEvent>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(sent, 1, "the scene should play");
        assert!(
            matches!(world.resource::<NextState<Scene>>(), NextState::Unchanged),
            "scene-only exits must not transfer"
        );
        assert!(world.get_resource::<PendingArrival>().is_none());
        assert!(
            world.get_resource::<PendingTransferAfterDialogue>().is_none(),
            "no deferred transfer either - the player stays at the table"
        );
    }

    /// RPGMaker's checkEventTriggerThere: an action event also activates
    /// when the player FACES it from the adjacent tile. Amy walked up to
    /// the retro table, faced the parchment, pressed E, and nothing
    /// happened until she happened to be standing on the trigger itself.
    #[test]
    fn action_exit_fires_when_faced_from_one_tile_away() {
        // Player one tile SOUTH of the trigger, facing Up (the harness
        // default) - looking straight at it.
        let mut world = setup_world((12, 13), retro_action_exit(), 24, 21);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);

        world.run_system_once(check_map_exits).unwrap();

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::End)),
            "E while facing the action tile should transfer, got {next:?}"
        );
    }

    #[test]
    fn action_exit_ignores_the_press_when_facing_away() {
        let mut world = setup_world((12, 13), retro_action_exit(), 24, 21);
        // Turn the player's back to the trigger.
        let player = world
            .query_filtered::<Entity, With<Player>>()
            .single(&world)
            .unwrap();
        *world.get_mut::<crate::player::Facing>(player).unwrap() =
            crate::player::Facing::Down;
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);

        world.run_system_once(check_map_exits).unwrap();

        assert!(
            matches!(world.resource::<NextState<Scene>>(), NextState::Unchanged),
            "E with the trigger behind the player must do nothing"
        );
    }

    #[test]
    fn action_exit_does_not_fire_on_touch() {
        // Standing right on the action tile without pressing E must do
        // nothing - this is the "walk near the inn table, get warped to
        // the goodbye scene" bug.
        let mut world = setup_world((12, 12), retro_action_exit(), 24, 21);

        world.run_system_once(check_map_exits).unwrap();

        assert!(matches!(world.resource::<NextState<Scene>>(), NextState::Unchanged));
        assert!(world.get_resource::<PendingArrival>().is_none());
    }

    #[test]
    fn action_exit_fires_on_e_press_while_standing_on_it() {
        let mut world = setup_world((12, 12), retro_action_exit(), 24, 21);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);

        world.run_system_once(check_map_exits).unwrap();

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::End)),
            "E on the action tile should transfer, got {next:?}"
        );
    }

    #[test]
    fn exit_with_dialogue_plays_the_scene_before_transferring() {
        // The retro dialog: E on the tile must start the scripted scene
        // (StartDialogueEvent) and defer the transfer, not jump straight
        // to End - and once the scene closes, the deferred transfer fires.
        let mut exits = retro_action_exit();
        exits[0].dialogue = vec![crate::map_data::DialogueSegmentData {
            speaker: "Nyaanager Evie".into(),
            portrait: "Nature".into(),
            face_index: 4,
            text: "Thanks for helping us with this incident Amy.".into(),
        }];
        let mut world = setup_world((12, 12), exits, 24, 21);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);

        world.run_system_once(check_map_exits).unwrap();

        assert!(
            matches!(world.resource::<NextState<Scene>>(), NextState::Unchanged),
            "transfer must wait for the scripted scene"
        );
        assert!(world.get_resource::<PendingArrival>().is_none());
        assert!(world.get_resource::<PendingTransferAfterDialogue>().is_some());
        let sent = world
            .resource::<Messages<crate::dialogue::StartDialogueEvent>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(sent, 1, "the scripted scene should have been started");

        // Scene closes (Mode re-enters Exploring) -> deferred transfer fires.
        world.run_system_once(fire_transfer_after_dialogue).unwrap();

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::End)),
            "deferred transfer should fire after the scene, got {next:?}"
        );
        let arrival = world.resource::<PendingArrival>();
        assert_eq!((arrival.spawn_x, arrival.spawn_y), (8, 5));
        assert!(world.get_resource::<PendingTransferAfterDialogue>().is_none());
    }

    #[test]
    fn fire_transfer_after_dialogue_is_inert_without_a_pending_transfer() {
        // OnEnter(Mode::Exploring) fires at game start and after every
        // ordinary NPC dialogue - it must do nothing then.
        let mut world = World::new();
        world.init_resource::<NextState<Scene>>();
        world.run_system_once(fire_transfer_after_dialogue).unwrap();
        assert!(matches!(world.resource::<NextState<Scene>>(), NextState::Unchanged));
        assert!(world.get_resource::<PendingArrival>().is_none());
    }

    #[test]
    fn bumping_into_an_exit_tile_triggers_it_without_standing_on_it() {
        // The town's door tiles are collision-blocked, so the player can
        // never STAND on one - the exit must fire from the bump (RPGMaker
        // "Player Touch"). Regression test for doors being unreachable:
        // walking into a door did nothing because only the standing tile
        // was ever checked.
        let mut world = setup_world((8, 30), town_of_endgame_exits(), TOWN_WIDTH, TOWN_HEIGHT);
        world.write_message(crate::player::BumpedIntoTile { tile_x: 8, tile_y: 29 });

        world.run_system_once(check_map_exits).unwrap();

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::TeamMarathonRetro)),
            "bump into the door tile should transfer, got {next:?}"
        );
    }

    #[test]
    fn door_on_exit_tile_opens_before_the_transition_fires() {
        // With a Door entity on the trigger tile, touching the exit must
        // NOT transition immediately: it starts the open choreography
        // (DepartingDoor) and the transition fires only after the door has
        // cycled through its opening rows plus the open-hold delay.
        let mut world = setup_world((8, 29), town_of_endgame_exits(), TOWN_WIDTH, TOWN_HEIGHT);
        world.init_resource::<Time>();
        world.spawn((
            Sprite::default(),
            Door { tile_x: 8, tile_y: 29, sprite_slot: 1, pattern: 1 },
        ));

        world.run_system_once(check_map_exits).unwrap();

        assert!(
            matches!(world.resource::<NextState<Scene>>(), NextState::Unchanged),
            "transition must wait for the door animation"
        );
        assert!(world.get_resource::<PendingArrival>().is_none());
        assert!(world.get_resource::<DepartingDoor>().is_some());

        // Play the whole choreography out: 3 row-steps at DOOR_STAGE_SECONDS
        // plus the open hold. Tick generously past the total.
        for _ in 0..12 {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            world.run_system_once(animate_door_departure).unwrap();
        }

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::TeamMarathonRetro)),
            "transition should fire after the door opens, got {next:?}"
        );
        let arrival = world.resource::<PendingArrival>();
        assert_eq!((arrival.spawn_x, arrival.spawn_y), (12, 15));
        // The resource deliberately lingers until despawn_map so player
        // input stays frozen through the actual scene swap - removing it
        // at transfer time gave one frame of free movement mid-transfer.
        assert!(
            world.get_resource::<DepartingDoor>().is_some(),
            "DepartingDoor must persist until the scene teardown clears it"
        );
    }

    // Intro's real, converted exit (assets/data/maps/intro.json) - the one
    // door out of the game's opening scene, back to Town of Endgame.
    fn intro_exits() -> Vec<ExitData> {
        vec![
            ExitData { trigger_x: 8, trigger_y: 1, target_scene: "TownOfEndgame".into(), target_spawn_x: 16, target_spawn_y: 23, trigger: ExitTrigger::Touch, dialogue: vec![], cancel_on_escape: false },
        ]
    }

    #[test]
    fn player_on_intro_door_triggers_town_of_endgame() {
        // Intro (Map010, 17x13) has exactly one exit in the original data:
        // walking onto tile (8, 1) transfers to Town of Endgame at (16,
        // 23). This pins that behavior so a future re-conversion or edit
        // can't silently break the game's very first scene transition.
        // Built directly (not via the module's `setup_world` helper, which
        // hardcodes Town of Endgame's 34x39 dimensions) so the CollisionMap
        // size matches Intro's real map.
        const WIDTH: u32 = 17;
        const HEIGHT: u32 = 13;

        let mut world = World::new();
        world.init_resource::<NextState<Scene>>();
        world.init_resource::<Messages<crate::player::BumpedIntoTile>>();
        world.init_resource::<Messages<crate::dialogue::StartDialogueEvent>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.insert_resource(MapExits(intro_exits()));
        world.insert_resource(CollisionMap::new(WIDTH, HEIGHT));
        let world_pos = tile_to_world(8, 1, WIDTH, HEIGHT);
        world.spawn((Player, crate::player::Facing::Up, Transform::from_xyz(world_pos.x, world_pos.y, 1.0)));

        world.run_system_once(check_map_exits).unwrap();

        let next = world.resource::<NextState<Scene>>();
        assert!(
            matches!(next, NextState::Pending(Scene::TownOfEndgame)),
            "expected a pending transition to TownOfEndgame, got {next:?}"
        );

        let arrival = world.resource::<PendingArrival>();
        assert_eq!((arrival.spawn_x, arrival.spawn_y), (16, 23));
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
                trigger: ExitTrigger::Touch,
                dialogue: vec![],
                cancel_on_escape: false,
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
