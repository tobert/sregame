use bevy::prelude::*;
use crate::game_state::GameState;
use crate::player::Player;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            camera_follow_player,
        ).run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct CameraFollow {
    pub smoothness: f32,
    pub bounds: Option<CameraBounds>,
}

impl Default for CameraFollow {
    fn default() -> Self {
        Self {
            smoothness: 5.0,
            bounds: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct CameraBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl CameraBounds {
    pub fn from_map_size(width: f32, height: f32, camera_half_width: f32, camera_half_height: f32) -> Self {
        let min_x = -width / 2.0 + camera_half_width;
        let max_x = width / 2.0 - camera_half_width;
        let min_y = -height / 2.0 + camera_half_height;
        let max_y = height / 2.0 - camera_half_height;

        Self {
            min_x: min_x.min(max_x),
            max_x: max_x.max(min_x),
            min_y: min_y.min(max_y),
            max_y: max_y.max(min_y),
        }
    }

    fn clamp(&self, mut position: Vec3) -> Vec3 {
        position.x = position.x.clamp(self.min_x, self.max_x);
        position.y = position.y.clamp(self.min_y, self.max_y);
        position
    }
}

fn camera_follow_player(
    mut camera_query: Query<(&mut Transform, &CameraFollow), (With<MainCamera>, Without<Player>)>,
    player_query: Query<&Transform, With<Player>>,
    time: Res<Time>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let Ok((mut camera_transform, follow_config)) = camera_query.single_mut() else {
        return;
    };

    let mut target = player_transform.translation;
    target.z = 999.9;

    if let Some(bounds) = follow_config.bounds {
        target = bounds.clamp(target);
    }

    let lerp_factor = follow_config.smoothness * time.delta_secs();
    let lerp_factor = lerp_factor.clamp(0.0, 1.0);

    camera_transform.translation = camera_transform.translation.lerp(target, lerp_factor);

    camera_transform.translation.z = 999.9;
}
