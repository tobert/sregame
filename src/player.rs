use bevy::prelude::*;
use crate::game_state::{GameState, Mode};
use crate::tilemap::CollisionMap;
use crate::assets::GameAssets;
use crate::instrumentation::{GameTracer, PlayerSessionTrace};

pub struct PlayerPlugin;

/// Label for the player input->movement->animation chain so downstream
/// consumers (transitions.rs reads the post-move position and this frame's
/// bump messages) can order themselves after it instead of racing it a
/// frame behind. (kaibo review 2026-07-12, finding agreed by both
/// reviewers.)
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerMovementSet;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Player>()
            .register_type::<Velocity>()
            .register_type::<Facing>()
            .add_message::<BumpedIntoTile>()
            .add_systems(OnEnter(GameState::Playing), spawn_player)
            .add_systems(Update, (
                player_movement_input,
                apply_movement,
                animate_player,
            ).chain().in_set(PlayerMovementSet).run_if(in_state(Mode::Exploring)));
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Player;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Velocity(pub Vec2);

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub enum Facing {
    #[default]
    Down,
    Left,
    Right,
    Up,
}

impl Facing {
    fn sprite_row(&self) -> u32 {
        match self {
            Facing::Down => 0,
            Facing::Left => 1,
            Facing::Right => 2,
            Facing::Up => 3,
        }
    }
}

/// Amy's slot in Amy-Walking.png (Actors.json: actor 1, characterIndex 0).
const AMY_SLOT: u32 = 0;

#[derive(Component)]
pub struct AnimationState {
    pub frame_timer: Timer,
    pub current_frame: usize,
    pub is_moving: bool,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            frame_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
            current_frame: 1,
            is_moving: false,
        }
    }
}

const PLAYER_SPEED: f32 = 150.0;

/// Emitted when the player tries to walk into a collision-blocked tile.
/// RPGMaker's "Player Touch" trigger fires on exactly this bump - the
/// town's door tiles are themselves impassable, so a door exit can only
/// ever fire via a bump, never by standing on its tile (see
/// transitions::check_map_exits).
#[derive(Message)]
pub struct BumpedIntoTile {
    pub tile_x: i32,
    pub tile_y: i32,
}

fn spawn_player(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    tracer: Option<Res<GameTracer>>,
    existing_players: Query<Entity, With<Player>>,
) {
    // Debug assertion: check for existing players before spawning
    #[cfg(debug_assertions)]
    {
        let player_count = existing_players.iter().count();
        if player_count > 0 {
            error!("❌ Attempting to spawn player when {} already exist! Potential duplicate spawn.", player_count);
        }
    }
    let texture = game_assets.player_sprite.clone();

    let atlas_layout = texture_atlas_layouts.add(crate::character_sheet::sheet_layout());

    // Create session trace for this play session (if telemetry is enabled)
    let session_trace = tracer.as_ref().map(|t| PlayerSessionTrace::new(t));

    if let Some(ref trace) = session_trace {
        info!("🎮 Player session started - trace ID: {:?}", trace.span_context().trace_id());
    } else {
        info!("🎮 Player session started (telemetry disabled)");
    }

    let mut entity_commands = commands.spawn((
        Player,
        Velocity(Vec2::ZERO),
        Facing::default(),
        AnimationState::default(),
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: crate::character_sheet::atlas_index(
                    AMY_SLOT,
                    Facing::default().sprite_row(),
                    crate::character_sheet::STANDING_PATTERN,
                ) as usize,
            },
        ),
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));

    // Attach session trace to player if telemetry is enabled
    if let Some(trace) = session_trace {
        entity_commands.insert(trace);
    }

    info!("Player (Amy) spawned at origin");
}

fn player_movement_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    departing: Option<Res<crate::transitions::DepartingDoor>>,
    mut query: Query<(&mut Velocity, &mut Facing, &mut AnimationState), With<Player>>,
) {
    let Ok((mut velocity, mut facing, mut anim_state)) = query.single_mut() else {
        return;
    };

    // Input freezes while a door-open departure plays out, like RPGMaker's
    // transfer lock.
    if departing.is_some() {
        velocity.0 = Vec2::ZERO;
        anim_state.is_moving = false;
        return;
    }

    let mut direction = Vec2::ZERO;

    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if direction.length_squared() > 0.0 {
        velocity.0 = direction.normalize() * PLAYER_SPEED;
        anim_state.is_moving = true;

        if direction.y.abs() > direction.x.abs() {
            *facing = if direction.y > 0.0 { Facing::Up } else { Facing::Down };
        } else if direction.x != 0.0 {
            *facing = if direction.x > 0.0 { Facing::Right } else { Facing::Left };
        }
    } else {
        velocity.0 = Vec2::ZERO;
        anim_state.is_moving = false;
        anim_state.current_frame = 1;
    }
}

fn apply_movement(
    time: Res<Time>,
    collision_map: Option<Res<CollisionMap>>,
    mut query: Query<(&Velocity, &mut Transform), With<Player>>,
    mut bumps: MessageWriter<BumpedIntoTile>,
) {
    for (velocity, mut transform) in &mut query {
        if velocity.0.length_squared() == 0.0 {
            continue;
        }

        let delta = velocity.0 * time.delta_secs();
        let mut position = transform.translation.truncate();

        // Each axis moves independently (RPGMaker has no diagonals; this
        // also gives wall-sliding: a diagonal push along a wall keeps the
        // free axis moving). Tile crossings go through can_step, which
        // honors directional passability - a plain "is the target tile
        // walkable" check can't represent one-way edges like the item
        // shop's counter.
        for axis_delta in [Vec2::new(delta.x, 0.0), Vec2::new(0.0, delta.y)] {
            if axis_delta == Vec2::ZERO {
                continue;
            }
            let candidate = position + axis_delta;

            let allowed = if let Some(collision_map) = &collision_map {
                match try_axis_move(collision_map, position, candidate, COLLIDER_HALF) {
                    Ok(()) => true,
                    Err((tile_x, tile_y)) => {
                        bumps.write(BumpedIntoTile { tile_x, tile_y });
                        false
                    }
                }
            } else {
                true
            };

            if allowed {
                position = candidate;
            }
        }

        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

/// Collision box half-extents. The sprite is 48px square, but colliding its
/// full extent would snag on every doorframe; colliding only the center
/// point (as an earlier version did) buried the sprite 24px deep into wall
/// sides while sliding along them - the "clip through the side of wall
/// panels" report, confirmed by the 2026-07-12 kaibo review. A slightly
/// narrow box keeps the body out of walls while slipping through one-tile
/// doorways comfortably.
const COLLIDER_HALF: Vec2 = Vec2::new(14.0, 12.0);

/// Attempts one axis-aligned sub-tile move of the collision box: the two
/// leading corners each check their own tile crossing through canPass.
/// Returns the first blocking tile on failure (for bump-triggered exits).
fn try_axis_move(
    map: &CollisionMap,
    position: Vec2,
    candidate: Vec2,
    half: Vec2,
) -> Result<(), (i32, i32)> {
    let delta = candidate - position;
    let probes: [Vec2; 2] = if delta.x != 0.0 {
        let lead_x = half.x.copysign(delta.x);
        [Vec2::new(lead_x, -half.y), Vec2::new(lead_x, half.y)]
    } else {
        let lead_y = half.y.copysign(delta.y);
        [Vec2::new(-half.x, lead_y), Vec2::new(half.x, lead_y)]
    };

    for probe in probes {
        let from = crate::map_data::world_to_tile(position + probe, map.width, map.height);
        let to = crate::map_data::world_to_tile(candidate + probe, map.width, map.height);
        if !map.can_step(from, to) {
            return Err(to);
        }
    }
    Ok(())
}

fn animate_player(
    time: Res<Time>,
    mut query: Query<(&mut AnimationState, &Facing, &mut Sprite), With<Player>>,
) {
    for (mut anim_state, facing, mut sprite) in &mut query {
        if !anim_state.is_moving {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = crate::character_sheet::atlas_index(
                    AMY_SLOT,
                    facing.sprite_row(),
                    crate::character_sheet::STANDING_PATTERN,
                ) as usize;
            }
            continue;
        }

        anim_state.frame_timer.tick(time.delta());

        if anim_state.frame_timer.just_finished() {
            // Same ping-pong walk cycle as NPC stepping (0,1,2,1) - a plain
            // 0,1,2 sawtooth skips the return-to-middle frame and reads as
            // a stutter (kaibo review 2026-07-12).
            anim_state.current_frame = (anim_state.current_frame + 1) % 4;

            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = crate::character_sheet::atlas_index(
                    AMY_SLOT,
                    facing.sprite_row(),
                    crate::npc::step_pattern(anim_state.current_frame as u8),
                ) as usize;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tilemap::TileCollision;

    // 3x3 all-open map with a blocked center column cell at (1,1).
    fn map_with_wall() -> CollisionMap {
        let mut map = CollisionMap::new(3, 3);
        map.set_tile(1, 1, TileCollision::Blocked);
        map
    }

    #[test]
    fn box_corners_stop_at_wall_sides_where_center_point_slipped_past() {
        // Player in tile (0,1) (world x approx -48..0 band on a 3x3 map),
        // vertically offset so the CENTER sits in row (0,0)'s band but the
        // lower corner hangs into row 1 - moving right, the old
        // center-point check would pass (center crosses into open (1,0))
        // while the sprite's lower half visibly clipped into the (1,1)
        // wall. The corner probe must refuse.
        let map = map_with_wall();
        // 3x3 map: tile (1,1) center is world (0,0), tiles 48px, so column
        // 1 spans x in [-24, 24] and row 1 spans y in [-24, 24].
        //
        // Center at y=+18: the center's own row is 0 (open), but the lower
        // corner (y-12 = +6) hangs into blocked row 1. Moving right, the
        // leading corner (x+14) crosses the column boundary at x=-24
        // (center crossing x=-38): the old center-point check would happily
        // continue until the center itself crossed at x=-24, burying the
        // sprite 14px+ deep into the wall.
        let from = Vec2::new(-40.0, 18.0);
        let to = Vec2::new(-37.0, 18.0); // corner moves -26 -> -23: crosses into column 1
        let result = try_axis_move(&map, from, to, COLLIDER_HALF);
        assert_eq!(result, Err((1, 1)), "lower-leading corner must hit the wall");
    }

    #[test]
    fn box_fits_through_a_one_tile_doorway() {
        // Column x=1 fully blocked except a doorway at (1,1); the 28px-wide
        // box must pass through the 48px doorway when reasonably centered.
        let mut map = CollisionMap::new(3, 3);
        map.set_tile(1, 0, TileCollision::Blocked);
        map.set_tile(1, 2, TileCollision::Blocked);

        // Walk right through the doorway row: tile (1,1) center is (0,0).
        let from = Vec2::new(-30.0, 0.0);
        let to = Vec2::new(-20.0, 0.0);
        assert_eq!(try_axis_move(&map, from, to, COLLIDER_HALF), Ok(()));
    }

    #[test]
    fn box_blocked_when_off_center_in_doorway() {
        // Same doorway, but hugging the doorway's top edge so the upper
        // corner would cross into the blocked (1,0): refused.
        let mut map = CollisionMap::new(3, 3);
        map.set_tile(1, 0, TileCollision::Blocked);
        map.set_tile(1, 2, TileCollision::Blocked);

        // Center at y=16: upper corner (y+12 = 28) is in blocked row 0.
        // Leading corner (x+14) crosses the column-1 boundary at x=-24 when
        // the move goes -42 -> -36 (corner -28 -> -22).
        let from = Vec2::new(-42.0, 16.0);
        let to = Vec2::new(-36.0, 16.0);
        assert_eq!(try_axis_move(&map, from, to, COLLIDER_HALF), Err((1, 0)));
    }
}
