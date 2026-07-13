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

    /// Tile-space offset of the cell the player is looking at (RPGMaker
    /// orientation: +y is down). Used by interaction reach (counters,
    /// facing-adjacent action exits).
    pub fn tile_delta(&self) -> (i32, i32) {
        match self {
            Facing::Down => (0, 1),
            Facing::Left => (-1, 0),
            Facing::Right => (1, 0),
            Facing::Up => (0, -1),
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
        crate::depth::YSorted { foot_offset: -24.0 },
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
    npcs: Query<&Transform, (With<crate::npc::NpcBody>, Without<Player>)>,
    mut bumps: MessageWriter<BumpedIntoTile>,
) {
    let npc_centers: Vec<Vec2> = npcs.iter().map(|t| t.translation.truncate()).collect();

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
                match try_axis_move(collision_map, position, candidate, COLLIDER_OFFSET, COLLIDER_HALF) {
                    Ok(()) => true,
                    Err((tile_x, tile_y)) => {
                        bumps.write(BumpedIntoTile { tile_x, tile_y });
                        false
                    }
                }
            } else {
                true
            } && !npc_blocks_move(&npc_centers, position, candidate);

            if allowed {
                position = candidate;
            }
        }

        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

/// NPC body collider half-extents/offset, relative to the NPC's translation
/// (its tile center). NPCs used to bake their whole 48px tile into the
/// CollisionMap, which read as a boundary "wider than it needs to be while
/// also shorter than it should be" (Amy's Team Disco entrance report): a
/// full-tile block stops the player ~8px of daylight away sideways, yet
/// vertically the short player box let the sprites overlap head-to-feet.
/// This box is body-shaped instead: 32px wide so bodies stop just about
/// touching; bottom edge at the NPC's feet (-24) so a player approaching
/// from the south stops close and renders in front (depth.rs); top edge at
/// +16 so a player approaching from the north keeps their feet off the
/// NPC's head while their lower body may overlap behind it.
const NPC_COLLIDER_HALF: Vec2 = Vec2::new(16.0, 20.0);
const NPC_COLLIDER_OFFSET: Vec2 = Vec2::new(0.0, -4.0);

/// True when the candidate position would push the player's collision box
/// into an NPC body it isn't already overlapping. The "already overlapping"
/// escape hatch means a player who somehow ends up inside an NPC (a scene
/// spawn point on a body, a future moving NPC walking into them) can always
/// walk out instead of being wedged forever.
fn npc_blocks_move(npc_centers: &[Vec2], position: Vec2, candidate: Vec2) -> bool {
    let overlaps = |player_center: Vec2, npc_center: Vec2| {
        let gap = ((player_center + COLLIDER_OFFSET) - (npc_center + NPC_COLLIDER_OFFSET)).abs();
        gap.x < COLLIDER_HALF.x + NPC_COLLIDER_HALF.x && gap.y < COLLIDER_HALF.y + NPC_COLLIDER_HALF.y
    };
    npc_centers
        .iter()
        .any(|&npc| overlaps(candidate, npc) && !overlaps(position, npc))
}

/// Collision box half-extents. The sprite is 48px square, but colliding its
/// full extent would snag on every doorframe; colliding only the center
/// point (as an earlier version did) buried the sprite 24px deep into wall
/// sides while sliding along them - the "clip through the side of wall
/// panels" report, confirmed by the 2026-07-12 kaibo review. A slightly
/// narrow box keeps the body out of walls while slipping through one-tile
/// doorways comfortably.
const COLLIDER_HALF: Vec2 = Vec2::new(14.0, 8.0);

/// The box is anchored at the sprite's FEET, not its center: it spans
/// y in [-24, -8] relative to the sprite. A center-anchored box let the
/// legs render 12px inside the top half of wall bands when walking south
/// against them (Amy's item-shop report, take two); with the box bottom
/// flush with the sprite bottom, walking south stops with zero overlap,
/// and walking north lets the head overlap a wall FACE - which reads as
/// standing in front of the wall, the perspective a top-down interior
/// implies. Overlaps between characters are depth-sorted (see depth.rs).
const COLLIDER_OFFSET: Vec2 = Vec2::new(0.0, -16.0);

/// The point that decides which tile the player logically occupies: the
/// collision box center, NOT the sprite center. Walking north, collision
/// lets the sprite center penetrate up to 8px into the blocked row (the
/// head-overlaps-the-wall-face perspective effect above) - deriving the
/// tile from the sprite center then reports the player as standing INSIDE
/// the wall/table row, and interactions like action exits stop matching
/// (Amy's report: E did nothing flush against the retro table, but worked
/// after "bumping back just a touch"). The box center always stays inside
/// the true standing tile.
pub fn logical_position(translation: Vec2) -> Vec2 {
    translation + COLLIDER_OFFSET
}

/// Attempts one axis-aligned sub-tile move of the collision box: the two
/// leading corners each check their own tile crossing through canPass.
/// Returns the first blocking tile on failure (for bump-triggered exits).
fn try_axis_move(
    map: &CollisionMap,
    position: Vec2,
    candidate: Vec2,
    offset: Vec2,
    half: Vec2,
) -> Result<(), (i32, i32)> {
    let delta = candidate - position;
    let probes: [Vec2; 2] = if delta.x != 0.0 {
        let lead_x = offset.x + half.x.copysign(delta.x);
        [
            Vec2::new(lead_x, offset.y - half.y),
            Vec2::new(lead_x, offset.y + half.y),
        ]
    } else {
        let lead_y = offset.y + half.y.copysign(delta.y);
        [
            Vec2::new(offset.x - half.x, lead_y),
            Vec2::new(offset.x + half.x, lead_y),
        ]
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
        // 3x3 map: tile (1,1) center is world (0,0), tiles 48px, so column
        // 1 spans x in [-24, 24], row 0 spans y in [24, 72], row 1 spans
        // y in [-24, 24].
        //
        // The feet box spans y in [center-24, center-8]. Sprite center at
        // y=+40 puts the box straddling the row 0/1 boundary: top corner
        // (y=32) in open row 0, bottom corner (y=16) hanging into blocked
        // row 1. Moving right, the leading edge (x+14) crosses the column
        // boundary at x=-24 - the old center-point check would happily
        // continue until the center itself crossed, burying the sprite
        // 14px+ deep into the wall. The corner probe must refuse.
        let map = map_with_wall();
        let from = Vec2::new(-40.0, 40.0);
        let to = Vec2::new(-37.0, 40.0); // leading edge -26 -> -23: crosses into column 1
        let result = try_axis_move(&map, from, to, COLLIDER_OFFSET, COLLIDER_HALF);
        assert_eq!(result, Err((1, 1)), "lower-leading corner must hit the wall");
    }

    #[test]
    fn walking_south_stops_with_feet_flush_on_the_wall_top() {
        // Amy's item-shop report, take two: walking south against a
        // partition, the old center-anchored box stopped with the sprite's
        // legs 12px inside the wall's top half. The feet-anchored box must
        // stop with the sprite bottom (center - 24) exactly on the wall's
        // top edge (y=24 for the (1,1) wall) - flush, zero overlap.
        let map = map_with_wall();
        let flush = Vec2::new(0.0, 48.0); // sprite bottom at exactly y=24
        assert_eq!(
            try_axis_move(&map, Vec2::new(0.0, 52.0), flush, COLLIDER_OFFSET, COLLIDER_HALF),
            Ok(()),
            "descending TO the flush position must be allowed"
        );
        assert_eq!(
            try_axis_move(&map, flush, Vec2::new(0.0, 44.0), COLLIDER_OFFSET, COLLIDER_HALF),
            Err((1, 1)),
            "descending PAST the flush position must be refused"
        );
    }

    #[test]
    fn box_fits_through_a_one_tile_doorway() {
        // Column x=1 fully blocked except a doorway at (1,1); the 28px-wide
        // box must pass through the 48px doorway when reasonably centered.
        let mut map = CollisionMap::new(3, 3);
        map.set_tile(1, 0, TileCollision::Blocked);
        map.set_tile(1, 2, TileCollision::Blocked);

        // Walk right through the doorway row: tile (1,1) center is (0,0),
        // and a sprite standing dead-center has its feet box (y in
        // [-24, -8]) entirely inside the doorway row.
        let from = Vec2::new(-30.0, 0.0);
        let to = Vec2::new(-20.0, 0.0);
        assert_eq!(try_axis_move(&map, from, to, COLLIDER_OFFSET, COLLIDER_HALF), Ok(()));
    }

    #[test]
    fn box_blocked_when_feet_hang_into_the_doorway_frame() {
        // Same doorway, but riding high enough that the box's top corner
        // (center - 8) is inside the blocked (1,0) row band (y >= 24):
        // refused.
        let mut map = CollisionMap::new(3, 3);
        map.set_tile(1, 0, TileCollision::Blocked);
        map.set_tile(1, 2, TileCollision::Blocked);

        // Center at y=34: top corner y=26 is in blocked row 0. Leading
        // corner (x+14) crosses the column-1 boundary at x=-24 when the
        // move goes -42 -> -36 (corner -28 -> -22).
        let from = Vec2::new(-42.0, 34.0);
        let to = Vec2::new(-36.0, 34.0);
        assert_eq!(try_axis_move(&map, from, to, COLLIDER_OFFSET, COLLIDER_HALF), Err((1, 0)));
    }

    #[test]
    fn npc_body_stops_sideways_approach_where_bodies_touch() {
        // NPC at the origin: its body box spans x in [-16, 16]. The player
        // box (half-width 14) must stop with centers 30px apart - bodies
        // visually touching - instead of the full-tile 38px the old
        // CollisionMap bake enforced ("wider than it needs to be").
        let npc = vec![Vec2::ZERO];
        let from = Vec2::new(-32.0, 0.0);
        assert!(!npc_blocks_move(&npc, from, Vec2::new(-30.5, 0.0)));
        assert!(npc_blocks_move(&npc, from, Vec2::new(-29.0, 0.0)));
    }

    #[test]
    fn npc_body_stops_vertical_approaches_at_body_height() {
        // "...while also being shorter than it should be": the old
        // full-tile bake plus the short player box let the sprites overlap
        // head-to-feet vertically. The body box spans y in [-24, 16]
        // around the NPC: from the south the player stops with centers 16px
        // apart (close, rendering in front via y-sort); from the north the
        // player's feet (center - 24) stop on the box top (y=16), off the
        // NPC's head.
        let npc = vec![Vec2::ZERO];

        // From the south: blocked once the player center passes y=-16.
        let from_south = Vec2::new(0.0, -20.0);
        assert!(!npc_blocks_move(&npc, from_south, Vec2::new(0.0, -17.0)));
        assert!(npc_blocks_move(&npc, from_south, Vec2::new(0.0, -14.0)));

        // From the north: blocked once the player center passes y=40.
        let from_north = Vec2::new(0.0, 44.0);
        assert!(!npc_blocks_move(&npc, from_north, Vec2::new(0.0, 41.0)));
        assert!(npc_blocks_move(&npc, from_north, Vec2::new(0.0, 38.0)));
    }

    #[test]
    fn player_already_inside_an_npc_can_always_walk_out() {
        // The escape hatch: a player overlapping an NPC body (bad spawn
        // point, future moving NPCs) must never be wedged - moves are only
        // refused when they'd create a NEW overlap.
        let npc = vec![Vec2::ZERO];
        let inside = Vec2::new(0.0, 0.0);
        assert!(!npc_blocks_move(&npc, inside, Vec2::new(0.0, -6.0)));
        assert!(!npc_blocks_move(&npc, inside, Vec2::new(6.0, 0.0)));
    }
}
