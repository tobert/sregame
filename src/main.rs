use bevy::prelude::*;

mod game_state;
mod player;
mod camera;
mod tilemap;
mod dialogue;

use game_state::{GameState, GameStatePlugin, Scene};
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};
use tilemap::TilemapPlugin;
use dialogue::{DialoguePlugin, StartDialogueEvent};

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
            TilemapPlugin,
            DialoguePlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Loading), on_enter_loading)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(GameState::Dialogue), on_enter_dialogue)
        .add_systems(Update, test_dialogue_trigger.run_if(in_state(GameState::Playing)))
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

fn on_enter_loading(
    mut next_state: ResMut<NextState<GameState>>,
    mut next_scene: ResMut<NextState<Scene>>,
) {
    info!("Entering Loading state");
    next_state.set(GameState::Playing);
    next_scene.set(Scene::TownOfEndgame);
}

fn on_enter_playing() {
    info!("Entering Playing state - player can explore");
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}

fn test_dialogue_trigger(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dialogue_events: MessageWriter<StartDialogueEvent>,
    asset_server: Res<AssetServer>,
    dialogue_assets: Res<Assets<dialogue::DialogueData>>,
    mut dialogue_handle: Local<Option<Handle<dialogue::DialogueData>>>,
) {
    if dialogue_handle.is_none() {
        *dialogue_handle = Some(asset_server.load("data/dialogue/test_evie.dialogue.json"));
    }

    if keyboard.just_pressed(KeyCode::KeyD) {
        if let Some(handle) = dialogue_handle.as_ref() {
            if let Some(dialogue_data) = dialogue_assets.get(handle) {
                info!("Triggering dialogue from JSON: {}", dialogue_data.speaker);

                let portrait = dialogue_data.portrait.as_ref().map(|p| {
                    asset_server.load(format!("textures/portraits/{}", p))
                });

                dialogue_events.write(StartDialogueEvent {
                    speaker: dialogue_data.speaker.clone(),
                    portrait,
                    lines: dialogue_data.lines.clone(),
                });
            } else {
                warn!("Dialogue asset not yet loaded");
            }
        }
    }
}
