use bevy::prelude::*;
use crate::game_state::GameState;
use crate::tilemap::CollisionMap;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_player)
            .add_systems(Update, (
                player_movement_input,
                apply_movement,
                animate_player,
            ).chain().run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Velocity(pub Vec2);

#[derive(Component, Default)]
pub enum Facing {
    #[default]
    Down,
    Left,
    Right,
    Up,
}

impl Facing {
    fn sprite_row(&self) -> usize {
        match self {
            Facing::Down => 0,
            Facing::Left => 1,
            Facing::Right => 2,
            Facing::Up => 3,
        }
    }
}

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

fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let texture = asset_server.load("textures/characters/Amy-Walking.png");

    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(48, 48),
        3,
        4,
        None,
        None,
    );
    let atlas_layout = texture_atlas_layouts.add(layout);

    commands.spawn((
        Player,
        Velocity(Vec2::ZERO),
        Facing::default(),
        AnimationState::default(),
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: 1,
            },
        ),
        Transform::from_xyz(0.0, 0.0, 1.0)
            .with_scale(Vec3::splat(2.0)),
    ));

    info!("Player (Amy) spawned at origin");
}

fn player_movement_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Velocity, &mut Facing, &mut AnimationState), With<Player>>,
) {
    let Ok((mut velocity, mut facing, mut anim_state)) = query.single_mut() else {
        return;
    };

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
) {
    const TILE_SIZE: f32 = 48.0;

    for (velocity, mut transform) in &mut query {
        if velocity.0.length_squared() == 0.0 {
            continue;
        }

        let delta_x = velocity.0.x * time.delta_secs();
        let delta_y = velocity.0.y * time.delta_secs();
        let new_x = transform.translation.x + delta_x;
        let new_y = transform.translation.y + delta_y;

        let can_move = if let Some(collision_map) = &collision_map {
            let tile_x = ((new_x / TILE_SIZE) + (collision_map.width as f32 / 2.0)) as i32;
            let tile_y = ((new_y / TILE_SIZE) + (collision_map.height as f32 / 2.0)) as i32;

            collision_map.is_walkable(tile_x, tile_y)
        } else {
            true
        };

        if can_move {
            transform.translation.x = new_x;
            transform.translation.y = new_y;
        }
    }
}

fn animate_player(
    time: Res<Time>,
    mut query: Query<(&mut AnimationState, &Facing, &mut Sprite), With<Player>>,
) {
    for (mut anim_state, facing, mut sprite) in &mut query {
        if !anim_state.is_moving {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = facing.sprite_row() * 3 + 1;
            }
            continue;
        }

        anim_state.frame_timer.tick(time.delta());

        if anim_state.frame_timer.just_finished() {
            anim_state.current_frame = (anim_state.current_frame + 1) % 3;

            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = facing.sprite_row() * 3 + anim_state.current_frame;
            }
        }
    }
}
