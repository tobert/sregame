use bevy::prelude::*;

mod game_state;
mod assets;
mod player;
mod camera;
mod tilemap;
mod dialogue;
mod npc;
mod map_data;
mod telemetry;

use game_state::{GameState, GameStatePlugin, Scene};
use assets::AssetsPlugin;
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};
use tilemap::TilemapPlugin;
use dialogue::DialoguePlugin;
use npc::NpcPlugin;

fn main() {
    // Initialize OpenTelemetry BEFORE Bevy app
    // This sets up the tracing subscriber before Bevy's LogPlugin does
    let logger_provider = telemetry::init_telemetry()
        .expect("Failed to initialize OpenTelemetry");

    info!("🔭 OpenTelemetry initialized, sending logs to OTLP collector");

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
                // Disable Bevy's LogPlugin since we set up tracing ourselves
                .disable::<bevy::log::LogPlugin>()
        )
        .add_plugins((
            GameStatePlugin,
            AssetsPlugin,
            PlayerPlugin,
            CameraPlugin,
            TilemapPlugin,
            DialoguePlugin,
            NpcPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        .run();

    // Shutdown telemetry when app exits
    if let Err(e) = telemetry::shutdown_telemetry(logger_provider) {
        eprintln!("Failed to shutdown telemetry: {}", e);
    }
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

fn on_enter_playing(mut next_scene: ResMut<NextState<Scene>>) {
    info!("Entering Playing state - player can explore");
    next_scene.set(Scene::TownOfEndgame);
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}
