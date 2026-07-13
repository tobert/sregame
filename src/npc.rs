use bevy::prelude::*;
use crate::game_state::{GameState, Mode};
use crate::player::Player;
use crate::dialogue::StartDialogueEvent;
use crate::assets::GameAssets;
use crate::instrumentation::{GameTracer, GameMeter, PlayerSessionTrace, start_npc_interaction_span};
use opentelemetry::{KeyValue, trace::{Span as _, Tracer}};

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Npc>()
            .register_type::<NpcFacing>()
            .register_type::<NpcDialogue>()
            .register_type::<CharacterFrames>()
            .register_type::<Interactable>()
            .register_type::<NpcBody>()
            .add_systems(Update, (
                check_npc_proximity,
                handle_interaction_input,
            ).chain().run_if(in_state(Mode::Exploring)))
            // Wandering pauses during dialogue - doggo shouldn't stroll off
            // mid-"wan wan".
            .add_systems(Update, wander_npcs.run_if(in_state(Mode::Exploring)))
            // Stepping runs whenever the game is playing - in the original,
            // NPCs keep bobbing behind an open dialogue box too.
            .add_systems(Update, animate_stepping_npcs.run_if(in_state(GameState::Playing)));
    }
}

/// Marker: this NPC's body blocks the player. Inserted at spawn for every
/// NPC whose original event is NOT Through (all of them except doggo) -
/// player.rs::npc_blocks_move collides only against NpcBody carriers.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct NpcBody;

/// Random tile-step wandering (doggo). Steps one tile at a time in a
/// random direction, honoring map passability even though doggo is Through
/// in the source - engine-divergent (RPGMaker's Through ignores terrain)
/// but intent-faithful: a dog that wanders into the pond or through house
/// walls reads as a bug, not a feature. Amy's spec (2026-07-12): random
/// movement, not the original's scripted left/right patrol.
#[derive(Component)]
pub struct Wanderer {
    /// Pause between step decisions.
    idle: Timer,
    /// World-space destination of the step in progress, if any.
    target: Option<Vec2>,
    /// xorshift64 state, lazily seeded from the clock on first use.
    rng: u64,
}

impl Default for Wanderer {
    fn default() -> Self {
        Self {
            idle: Timer::from_seconds(1.5, TimerMode::Repeating),
            target: None,
            rng: 0,
        }
    }
}

/// RPGMaker move speed 3 (doggo's): 2^3/256 tiles per frame at 60fps on
/// 48px tiles = 90 px/s.
const WANDER_SPEED: f32 = 90.0;

fn wander_npcs(
    time: Res<Time>,
    collision_map: Option<Res<crate::tilemap::CollisionMap>>,
    mut query: Query<(&mut Wanderer, &mut Transform, &mut CharacterFrames)>,
) {
    let Some(map) = collision_map else { return };

    for (mut wanderer, mut transform, mut frames) in &mut query {
        // A step in progress: glide to the target tile, snap on arrival.
        if let Some(target) = wanderer.target {
            let position = transform.translation.truncate();
            let step = WANDER_SPEED * time.delta_secs();
            if position.distance(target) <= step {
                transform.translation.x = target.x;
                transform.translation.y = target.y;
                wanderer.target = None;
            } else {
                let direction = (target - position).normalize_or_zero();
                transform.translation.x += direction.x * step;
                transform.translation.y += direction.y * step;
            }
            continue;
        }

        wanderer.idle.tick(time.delta());
        if !wanderer.idle.just_finished() {
            continue;
        }

        if wanderer.rng == 0 {
            // |1 keeps the seed nonzero (xorshift's absorbing state).
            wanderer.rng = time.elapsed().as_nanos() as u64 | 1;
        }
        wanderer.rng ^= wanderer.rng << 13;
        wanderer.rng ^= wanderer.rng >> 7;
        wanderer.rng ^= wanderer.rng << 17;

        // Direction deltas in RPGMaker tile orientation (y grows downward).
        let (dx, dy, facing) = match wanderer.rng % 4 {
            0 => (0, 1, NpcFacing::Down),
            1 => (-1, 0, NpcFacing::Left),
            2 => (1, 0, NpcFacing::Right),
            _ => (0, -1, NpcFacing::Up),
        };

        let position = transform.translation.truncate();
        let from = crate::map_data::world_to_tile(position, map.width, map.height);
        let to = (from.0 + dx, from.1 + dy);
        if !map.can_step(from, to) {
            // Blocked step: just turn toward it and wait for the next tick,
            // like a dog sniffing at a wall.
            frames.facing_row = facing as u32;
            continue;
        }

        frames.facing_row = facing as u32;
        wanderer.target = Some(crate::map_data::tile_to_world(
            to.0 as u32,
            to.1 as u32,
            map.width,
            map.height,
        ));
    }
}

/// Which sheet slot and facing row an entity's sprite frames come from -
/// everything `animate_stepping_npcs` needs to pick atlas indices. Carried
/// by NPCs and ambient props alike (props have no `Npc` component).
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct CharacterFrames {
    pub slot: u32,
    pub facing_row: u32,
}

/// RPGMaker's "Stepping Animation": cycle the walk patterns in place.
/// Present only on NPCs/props whose original event had stepAnime enabled.
#[derive(Component)]
pub struct StepAnimation {
    timer: Timer,
    step: u8,
}

impl Default for StepAnimation {
    fn default() -> Self {
        Self {
            // The original's events use move speed 3: a pattern step every
            // 18 - 2*3 = 12 frames at 60fps = 0.2s.
            timer: Timer::from_seconds(0.2, TimerMode::Repeating),
            step: 0,
        }
    }
}

/// RPGMaker's stationary walk cycle is a ping-pong through the middle
/// column: pattern 0, 1, 2, 1, 0, 1, ... (rmmz_objects.js pattern() renders
/// internal step 3 as pattern 1).
pub fn step_pattern(step: u8) -> u32 {
    [0, 1, 2, 1][(step % 4) as usize]
}

fn animate_stepping_npcs(
    time: Res<Time>,
    mut query: Query<(&CharacterFrames, &mut StepAnimation, &mut Sprite)>,
) {
    for (frames, mut anim, mut sprite) in &mut query {
        anim.timer.tick(time.delta());
        if !anim.timer.just_finished() {
            continue;
        }
        anim.step = (anim.step + 1) % 4;
        if let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = crate::character_sheet::atlas_index(
                frames.slot,
                frames.facing_row,
                step_pattern(anim.step),
            ) as usize;
        }
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Npc {
    pub name: String,
    pub sprite_facing: NpcFacing,
    /// Character slot (0-7) within the sprite sheet - see character_sheet.rs.
    pub sprite_slot: u32,
}

#[derive(Clone, Copy, Reflect, Default)]
pub enum NpcFacing {
    #[default]
    Down = 0,
    Left = 1,
    Right = 2,
    Up = 3,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct NpcDialogue {
    pub speaker: String,
    pub portrait_path: String,
    /// Which cell of the `portrait_path` face sheet to crop and display (see
    /// `DialogueData::face_index` in map_data.rs and the atlas built in
    /// `dialogue.rs::spawn_dialogue_ui`).
    pub portrait_face_index: u32,
    pub lines: Vec<String>,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Interactable {
    pub radius: f32,
    pub prompt: String,
}

impl Default for Interactable {
    fn default() -> Self {
        Self {
            radius: 64.0,
            prompt: "Press E to talk".to_string(),
        }
    }
}

#[derive(Component)]
struct InRange;

pub fn spawn_npc(
    commands: &mut Commands,
    _game_assets: &GameAssets,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    position: Vec3,
    sprite_handle: Handle<Image>,
    npc_data: Npc,
    step_anime: bool,
    dialogue: NpcDialogue,
    tracer: Option<&GameTracer>,
) -> Entity {
    let texture = sprite_handle;

    let atlas_layout = texture_atlas_layouts.add(crate::character_sheet::sheet_layout());

    let sprite_index = crate::character_sheet::atlas_index(
        npc_data.sprite_slot,
        npc_data.sprite_facing as u32,
        crate::character_sheet::STANDING_PATTERN,
    ) as usize;

    // Add telemetry for NPC spawn
    if let Some(t) = tracer {
        let mut span = t.tracer().start("npc.spawned");
        span.set_attribute(KeyValue::new("npc.name", npc_data.name.clone()));
        span.set_attribute(KeyValue::new("npc.x", position.x as f64));
        span.set_attribute(KeyValue::new("npc.y", position.y as f64));
        span.set_attribute(KeyValue::new("npc.sprite_index", sprite_index as i64));
        span.end();
    }

    info!("👤 NPC spawned: {} at ({:.0}, {:.0})", npc_data.name, position.x, position.y);

    let frames = CharacterFrames {
        slot: npc_data.sprite_slot,
        facing_row: npc_data.sprite_facing as u32,
    };

    let mut entity_commands = commands.spawn((
        npc_data,
        frames,
        dialogue,
        Interactable::default(),
        crate::depth::YSorted { foot_offset: -24.0 },
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: sprite_index,
            },
        ),
        Transform::from_translation(position),
    ));
    if step_anime {
        entity_commands.insert(StepAnimation::default());
    }
    entity_commands.id()
}

fn check_npc_proximity(
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    npc_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, Without<InRange>)>,
    in_range_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, With<InRange>)>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();

    for (entity, npc_transform, interactable) in &npc_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if distance <= interactable.radius {
            commands.entity(entity).insert(InRange);
        }
    }

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
    player_query: Query<(&Transform, &crate::player::Facing, Option<&PlayerSessionTrace>), With<Player>>,
    npc_query: Query<(&Transform, &NpcDialogue), (With<Npc>, With<InRange>)>,
    all_npcs: Query<(&Transform, &NpcDialogue), With<Npc>>,
    mut dialogue_events: MessageWriter<StartDialogueEvent>,
    map_exits: Option<Res<crate::tilemap::MapExits>>,
    collision_map: Option<Res<crate::tilemap::CollisionMap>>,
    tracer: Option<Res<GameTracer>>,
    meter: Option<Res<GameMeter>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    let Ok((player_transform, player_facing, session_trace)) = player_query.single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();
    // Tile logic uses the collision-box center, not the sprite center: the
    // sprite center can sit inside a wall/table row when pressed flush
    // against it (see player::logical_position).
    let logical_pos = crate::player::logical_position(player_pos);

    // An action exit under the player - or on the tile they're facing
    // (both activate it, see check_map_exits) - owns the E press. Without
    // this, E on/at the retro-dialog tile with an NPC in range would fire
    // both the scripted scene AND that NPC's dialogue in the same frame
    // (kaibo review 2026-07-12).
    if let (Some(exits), Some(map)) = (&map_exits, &collision_map) {
        let (tile_x, tile_y) = crate::map_data::world_to_tile(logical_pos, map.width, map.height);
        let (dx, dy) = player_facing.tile_delta();
        let claims_press = exits.0.iter().any(|exit| {
            exit.trigger == crate::map_data::ExitTrigger::Action
                && ((exit.trigger_x as i32 == tile_x && exit.trigger_y as i32 == tile_y)
                    || (exit.trigger_x as i32 == tile_x + dx
                        && exit.trigger_y as i32 == tile_y + dy))
        });
        if claims_press {
            return;
        }
    }

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

    // Counter reach (RPGMaker Game_Player.checkEventTriggerThere): with
    // nobody in plain interaction range, if the tile directly ahead is a
    // counter, the press reaches the NPC one tile beyond it. This is how
    // shopkeepers behind counters are talkable - the 64px radius is
    // center-to-center and a counter puts ~96px between the two.
    let closest_npc = closest_npc.or_else(|| {
        let map = collision_map.as_ref()?;
        let (dx, dy) = player_facing.tile_delta();
        let (px, py) = crate::map_data::world_to_tile(logical_pos, map.width, map.height);
        if !map.is_counter(px + dx, py + dy) {
            return None;
        }
        let beyond = (px + 2 * dx, py + 2 * dy);
        all_npcs.iter().find_map(|(npc_transform, dialogue)| {
            let npc_pos = npc_transform.translation.truncate();
            let npc_tile = crate::map_data::world_to_tile(npc_pos, map.width, map.height);
            (npc_tile == beyond).then(|| (dialogue, player_pos.distance(npc_pos)))
        })
    });

    if let Some((dialogue, distance)) = closest_npc {
        info!("🤝 NPC interaction started: {} (distance: {:.1}px)", dialogue.speaker, distance);

        // Telemetry: Start NPC interaction span (if available)
        let telemetry_guard = if let (Some(tracer), Some(meter), Some(session_trace)) = (&tracer, &meter, session_trace) {
            let span = start_npc_interaction_span(
                tracer,
                session_trace,
                &dialogue.speaker,
                player_pos,
                distance,
            );

            // Record interaction metric
            meter.interactions_total.add(
                1,
                &[KeyValue::new("npc.name", dialogue.speaker.clone())]
            );

            // Set this span as the current context for dialogue event processing
            let context = opentelemetry::Context::current_with_value(span.span_context().clone());
            let guard = context.attach();
            Some((span, guard))
        } else {
            None
        };

        // One segment per paragraph, all sharing this NPC's speaker and
        // portrait (scripted scenes with per-box speakers come from exit
        // events instead - see transitions.rs).
        let segments = dialogue
            .lines
            .iter()
            .map(|line| crate::dialogue::DialogueSegment {
                speaker: dialogue.speaker.clone(),
                portrait_path: dialogue.portrait_path.clone(),
                portrait_face_index: dialogue.portrait_face_index,
                text: line.clone(),
            })
            .collect();

        dialogue_events.write(StartDialogueEvent { segments });

        // Clean up telemetry span if it was created
        if let Some((mut span, guard)) = telemetry_guard {
            drop(guard);
            span.end();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::map_data::tile_to_world;
    use crate::player::Facing;
    use crate::tilemap::CollisionMap;

    #[test]
    fn step_pattern_ping_pongs_through_the_middle() {
        // RPGMaker's stationary cycle: 0, 1, 2, 1, then wraps.
        let observed: Vec<u32> = (0..8).map(step_pattern).collect();
        assert_eq!(observed, vec![0, 1, 2, 1, 0, 1, 2, 1]);
    }

    #[test]
    fn step_pattern_never_leaves_the_slot_columns() {
        for step in 0..=u8::MAX {
            assert!(step_pattern(step) < 3, "step {step} escaped the 3 patterns");
        }
    }

    // A 5x5 world: player at (2,3) facing Up, an NPC at (2,1), and the tile
    // between them at (2,2) - a counter or not, per test.
    fn setup_counter_world(counter_between: bool) -> World {
        let mut world = World::new();
        world.init_resource::<Messages<StartDialogueEvent>>();
        world.init_resource::<ButtonInput<KeyCode>>();

        let mut map = CollisionMap::new(5, 5);
        if counter_between {
            map.counters.insert((2, 2));
        }
        world.insert_resource(map);

        let player_pos = tile_to_world(2, 3, 5, 5);
        world.spawn((
            Player,
            Facing::Up,
            Transform::from_xyz(player_pos.x, player_pos.y, 1.0),
        ));

        let npc_pos = tile_to_world(2, 1, 5, 5);
        world.spawn((
            Npc { name: "Isabella".into(), sprite_facing: NpcFacing::Down, sprite_slot: 0 },
            NpcDialogue {
                speaker: "Isabella".into(),
                portrait_path: String::new(),
                portrait_face_index: 0,
                lines: vec!["Welcome to the shop.".into()],
            },
            Transform::from_xyz(npc_pos.x, npc_pos.y, 1.0),
        ));

        world.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyE);
        world
    }

    fn dialogue_count(world: &World) -> usize {
        world
            .resource::<Messages<StartDialogueEvent>>()
            .iter_current_update_messages()
            .count()
    }

    #[test]
    fn counter_reach_talks_across_the_counter() {
        // The shopkeeper is two tiles away (~96px, outside the 64px radius,
        // so she never gets InRange), but the tile between is a counter:
        // E must reach across and start her dialogue - RPGMaker's
        // checkEventTriggerThere counter hop.
        let mut world = setup_counter_world(true);
        world.run_system_once(handle_interaction_input).unwrap();
        assert_eq!(dialogue_count(&world), 1, "counter should carry the press across");
    }

    #[test]
    fn no_counter_no_reach() {
        // Same layout without the counter flag: two tiles is simply out of
        // range and the press must do nothing.
        let mut world = setup_counter_world(false);
        world.run_system_once(handle_interaction_input).unwrap();
        assert_eq!(dialogue_count(&world), 0, "no counter, no long reach");
    }

    #[test]
    fn counter_reach_only_works_when_facing_the_counter() {
        // Facing AWAY from the counter (down) must not reach the NPC north
        // of it - the hop follows the player's facing, not proximity.
        let mut world = setup_counter_world(true);
        let mut facings = world.query_filtered::<&mut Facing, With<Player>>();
        *facings.single_mut(&mut world).unwrap() = Facing::Down;

        world.run_system_once(handle_interaction_input).unwrap();
        assert_eq!(dialogue_count(&world), 0, "reach must follow facing");
    }

    #[test]
    fn wanderer_steps_onto_a_walkable_tile_and_stops_at_walls() {
        // A wanderer on a 3x3 map whose center is the only walkable cell
        // can never leave it; once the ring opens up, a step decision picks
        // some adjacent walkable tile. Exercises the can_step gate with
        // every direction blocked vs. open.
        let mut world = World::new();
        world.init_resource::<Time>();

        let mut map = CollisionMap::new(3, 3);
        for x in 0..3 {
            for y in 0..3 {
                if (x, y) != (1, 1) {
                    map.set_tile(x, y, crate::tilemap::TileCollision::Blocked);
                }
            }
        }
        world.insert_resource(map);

        let center = tile_to_world(1, 1, 3, 3);
        world.spawn((
            Wanderer::default(),
            CharacterFrames { slot: 0, facing_row: 0 },
            Transform::from_xyz(center.x, center.y, 1.0),
        ));

        // Tick well past the idle timer several times: every step decision
        // must refuse (all four neighbors are blocked).
        for _ in 0..8 {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(2));
            world.run_system_once(wander_npcs).unwrap();
        }
        let mut wanderers = world.query::<(&Wanderer, &Transform)>();
        let (wanderer, transform) = wanderers.single(&world).unwrap();
        assert!(wanderer.target.is_none(), "boxed-in wanderer must not pick a target");
        assert_eq!(transform.translation.truncate(), center, "and must not move");

        // Open the ring: the next decision must pick an adjacent tile.
        world.insert_resource(CollisionMap::new(3, 3));
        let mut stepped = false;
        for _ in 0..8 {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(2));
            world.run_system_once(wander_npcs).unwrap();
            let mut wanderers = world.query::<&Wanderer>();
            if let Some(target) = wanderers.single(&world).unwrap().target {
                let neighbors: Vec<Vec2> = [(1u32, 0u32), (0, 1), (2, 1), (1, 2)]
                    .iter()
                    .map(|&(x, y)| tile_to_world(x, y, 3, 3))
                    .collect();
                assert!(
                    neighbors.contains(&target),
                    "wander target {target:?} is not an adjacent tile"
                );
                stepped = true;
                break;
            }
        }
        assert!(stepped, "an unboxed wanderer should step within a few ticks");
    }
}
