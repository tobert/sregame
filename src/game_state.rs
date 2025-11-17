use bevy::prelude::*;
use crate::instrumentation::ActiveDialogue;
use crate::dialogue::DialogueQueue;
use opentelemetry::{KeyValue, trace::Span as _};

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
    mut commands: Commands,
    active_dialogue: Option<ResMut<ActiveDialogue>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match current_state.get() {
            GameState::Dialogue => {
                info!("🚫 Force-exiting dialogue mode");

                // Clean up dialogue resources with telemetry
                if let Some(mut dialogue) = active_dialogue {
                    let chars_read = dialogue.chars_read;

                    // Add telemetry event for forced exit
                    dialogue.span.add_event(
                        "dialogue.forced_exit",
                        vec![
                            KeyValue::new("cleanup.type", "forced"),
                            KeyValue::new("dialogue.completed", false),
                            KeyValue::new("chars_read", chars_read as i64),
                        ],
                    );

                    info!("📊 Dialogue force-closed: {} chars read", chars_read);

                    // End span and remove resource
                    dialogue.span.end();
                    commands.remove_resource::<ActiveDialogue>();
                }

                // Remove dialogue queue
                commands.remove_resource::<DialogueQueue>();

                next_state.set(GameState::Playing);
            }
            _ => {}
        }
    }
}
