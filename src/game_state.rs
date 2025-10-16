use bevy::prelude::*;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
    Dialogue,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, SubStates)]
#[source(GameState = GameState::Playing)]
pub enum Scene {
    #[default]
    TownOfEndgame,
    TeamMarathon,
}

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<Scene>()
            .add_systems(Update, (
                debug_state_changes,
                handle_escape_key,
            ));
    }
}

fn debug_state_changes(
    state: Res<State<GameState>>,
) {
    if state.is_changed() {
        info!("Game state changed to: {:?}", state.get());
    }
}

fn handle_escape_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match current_state.get() {
            GameState::Dialogue => {
                info!("Exiting dialogue mode");
                next_state.set(GameState::Playing);
            }
            _ => {}
        }
    }
}
