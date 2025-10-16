use bevy::prelude::*;

mod game_state;
mod player;
mod camera;

use game_state::{GameState, GameStatePlugin};
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "The Endgame of SRE".to_string(),
                        resolution: (1920, 1080).into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                })
        )
        .add_plugins((
            GameStatePlugin,
            PlayerPlugin,
            CameraPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        .add_systems(Update, test_state_transitions.run_if(in_state(GameState::Playing)))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        CameraFollow::default(),
        Transform::from_xyz(0.0, 0.0, 999.9),
    ));

    info!("SRE Game initialized");
}

fn on_enter_loading(mut next_state: ResMut<NextState<GameState>>) {
    info!("Entering Loading state");
    next_state.set(GameState::Playing);
}

fn on_enter_playing() {
    info!("Entering Playing state - player can explore");
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}

fn test_state_transitions(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        info!("Testing transition to Dialogue state");
        next_state.set(GameState::Dialogue);
    }
}
