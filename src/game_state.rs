use bevy::prelude::*;
use crate::instrumentation::ActiveDialogue;
use crate::dialogue::DialogueQueue;
use opentelemetry::{KeyValue, trace::Span as _};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, SubStates)]
#[source(GameState = GameState::Playing)]
pub enum Scene {
    #[default]
    TownOfEndgame,
    TeamMarathon,
    TeamMarathonRetro,
    TeamDisco,
    TeamInferno,
    MahoganyRow,
    Intro,
    End,
}

/// Whether the player is freely exploring the current `Scene` or reading a
/// dialogue box. This is a *sibling* `SubState` to `Scene` - both are sourced
/// from `GameState::Playing` - rather than a variant of `GameState` itself.
///
/// This matters: `SubStates::should_exist` (see the derive macro output in
/// `bevy_state_macros`) always resets a sub-state to its `#[default]` variant
/// whenever its source state transitions into the matching value. If
/// `Dialogue` were a `GameState` sibling of `Playing`, then every dialogue
/// interaction would cause `GameState` to leave and re-enter `Playing`,
/// which would tear down and recreate `Scene` from scratch - despawning the
/// current map/NPCs and silently resetting the player back to
/// `Scene::TownOfEndgame` regardless of which scene they were actually in.
/// By keeping `GameState` at `Playing` throughout and toggling `Mode`
/// instead, `Scene`'s source condition never changes during dialogue, so it
/// is left completely alone.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, SubStates)]
#[source(GameState = GameState::Playing)]
pub enum Mode {
    #[default]
    Exploring,
    Dialogue,
}

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<Scene>()
            .add_sub_state::<Mode>()
            .add_systems(Update, (
                debug_state_changes,
                handle_escape_key.run_if(in_state(Mode::Dialogue)),
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

/// Force-exits dialogue mode. Gated on `run_if(in_state(Mode::Dialogue))` at
/// the call site, so this only ever runs while `Mode::Dialogue` is current.
fn handle_escape_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_mode: ResMut<NextState<Mode>>,
    mut commands: Commands,
    active_dialogue: Option<ResMut<ActiveDialogue>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }

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

    next_mode.set(Mode::Exploring);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    /// Regression test for "the scene disappears during dialog": entering
    /// and leaving `Mode::Dialogue` from a non-default `Scene` must leave
    /// `Scene` completely untouched.
    ///
    /// Before the fix, `Dialogue` was a variant of `GameState` sibling to
    /// `Playing`, and `Scene` was sourced from `GameState::Playing`. Every
    /// dialogue interaction made `GameState` leave `Playing` (killing
    /// `Scene`) and then re-enter it, which re-ran `Scene::should_exist` and
    /// reset it to its `#[default]` (`TownOfEndgame`) - silently teleporting
    /// the player out of whatever scene they were actually in. This test
    /// drives the exact same round trip (start dialogue, end dialogue)
    /// through the real `GameState`/`Scene`/`Mode` state machine and asserts
    /// `Scene` survives unchanged.
    #[test]
    fn dialogue_round_trip_from_non_default_scene_preserves_scene() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .add_sub_state::<Scene>()
            .add_sub_state::<Mode>();

        // Loading -> Playing, mirroring on_enter_playing/check_asset_loading.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert_eq!(*app.world().resource::<State<GameState>>().get(), GameState::Playing);
        assert_eq!(*app.world().resource::<State<Mode>>().get(), Mode::Exploring);

        // Navigate to a non-default scene, e.g. by walking through a map
        // exit (see transitions.rs::check_map_exits).
        app.world_mut()
            .resource_mut::<NextState<Scene>>()
            .set(Scene::TeamMarathonRetro);
        app.update();
        assert_eq!(*app.world().resource::<State<Scene>>().get(), Scene::TeamMarathonRetro);

        // Start a dialogue (mirrors dialogue::handle_dialogue_events setting
        // Mode::Dialogue on StartDialogueEvent).
        app.world_mut()
            .resource_mut::<NextState<Mode>>()
            .set(Mode::Dialogue);
        app.update();
        assert_eq!(*app.world().resource::<State<Mode>>().get(), Mode::Dialogue);
        // The core regression check: Scene must be untouched by entering dialogue.
        assert_eq!(*app.world().resource::<State<Scene>>().get(), Scene::TeamMarathonRetro);

        // End the dialogue (mirrors dialogue::advance_dialogue setting
        // Mode::Exploring once the queue is exhausted).
        app.world_mut()
            .resource_mut::<NextState<Mode>>()
            .set(Mode::Exploring);
        app.update();
        assert_eq!(*app.world().resource::<State<Mode>>().get(), Mode::Exploring);
        // The core regression check: Scene must still be TeamMarathonRetro,
        // NOT reset to the #[default] TownOfEndgame.
        assert_eq!(*app.world().resource::<State<Scene>>().get(), Scene::TeamMarathonRetro);
    }
}
