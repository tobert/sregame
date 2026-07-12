use bevy::prelude::*;
use crate::game_state::{GameState, Mode};
use crate::tilemap::CollisionMap;
use crate::assets::GameAssets;
use crate::instrumentation::{GameTracer, PlayerSessionTrace};

pub struct PlayerPlugin;

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
            ).chain().run_if(in_state(Mode::Exploring)));
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
                let from = crate::map_data::world_to_tile(
                    position, collision_map.width, collision_map.height);
                let to = crate::map_data::world_to_tile(
                    candidate, collision_map.width, collision_map.height);

                let ok = collision_map.can_step(from, to);
                if !ok {
                    bumps.write(BumpedIntoTile { tile_x: to.0, tile_y: to.1 });
                }
                ok
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
            anim_state.current_frame = (anim_state.current_frame + 1) % 3;

            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = crate::character_sheet::atlas_index(
                    AMY_SLOT,
                    facing.sprite_row(),
                    anim_state.current_frame as u32,
                ) as usize;
            }
        }
    }
}
