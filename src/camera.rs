use bevy::prelude::*;
use crate::game_state::GameState;
use crate::player::Player;

/// The game is designed around a 960x540 world-unit view - character/tile
/// proportions matching the RPGMaker original (~20x11 tiles on screen).
/// The camera projection uses `ScalingMode::AutoMin` with these dimensions,
/// so the full design view always fits the window: 2x pixel art on a
/// 1920x1080 window, scaled down to fit smaller canvases (e.g. the game
/// embedded inline in a blog post). Sprites and tiles both render at their
/// natural 48px world size. An earlier version instead scaled sprite
/// entities 2x with an unzoomed camera, which made characters two tiles
/// tall and the view twice as wide as intended.
pub const VIEW_WIDTH: f32 = 960.0;
pub const VIEW_HEIGHT: f32 = 540.0;

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

/// Half-extents of the current map, centered on the origin. The camera
/// half-extents are NOT baked in here: the visible area varies with window
/// size (AutoMin scaling), so clamping reads the projection's computed area
/// each frame instead of assuming the 960x540 design view.
#[derive(Clone, Copy)]
pub struct CameraBounds {
    pub map_half_width: f32,
    pub map_half_height: f32,
}

impl CameraBounds {
    pub fn from_map_size(width: f32, height: f32) -> Self {
        Self {
            map_half_width: width / 2.0,
            map_half_height: height / 2.0,
        }
    }

    /// Keep the view inside the map; if the view is larger than the map on
    /// an axis, pin the camera to the map's center on that axis.
    fn clamp(&self, mut position: Vec3, camera_half_size: Vec2) -> Vec3 {
        let max_x = (self.map_half_width - camera_half_size.x).max(0.0);
        let max_y = (self.map_half_height - camera_half_size.y).max(0.0);
        position.x = position.x.clamp(-max_x, max_x);
        position.y = position.y.clamp(-max_y, max_y);
        position
    }
}

fn camera_follow_player(
    mut camera_query: Query<(&mut Transform, &CameraFollow, &Projection), (With<MainCamera>, Without<Player>)>,
    player_query: Query<&Transform, With<Player>>,
    time: Res<Time>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let Ok((mut camera_transform, follow_config, projection)) = camera_query.single_mut() else {
        return;
    };

    let mut target = player_transform.translation;
    target.z = 999.9;

    if let Some(bounds) = follow_config.bounds {
        let Projection::Orthographic(ortho) = projection else {
            return;
        };
        target = bounds.clamp(target, ortho.area.half_size());
    }

    let lerp_factor = follow_config.smoothness * time.delta_secs();
    let lerp_factor = lerp_factor.clamp(0.0, 1.0);

    camera_transform.translation = camera_transform.translation.lerp(target, lerp_factor);

    camera_transform.translation.z = 999.9;
}
