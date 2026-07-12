use bevy::prelude::*;
use crate::game_state::{GameState, Scene};
use crate::player::Player;
use crate::instrumentation::PlayerSessionTrace;
use opentelemetry::trace::Span;
use opentelemetry::KeyValue;

/// Keeps the player's session span (see `PlayerSessionTrace`) annotated with
/// where the player is and which scene they're in, so an agent watching the
/// OTLP stream can answer "where is the player right now?" without BRP access.
///
/// Attributes on the span are overwritten every emit (cheap); span *events*
/// are only added when the state meaningfully changed, so an idle player
/// doesn't spam one event per second forever.
pub struct SemanticStatePlugin;

impl Plugin for SemanticStatePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(StateTraceTimer(Timer::from_seconds(1.0, TimerMode::Repeating)))
            .add_systems(Update, update_session_trace.run_if(in_state(GameState::Playing)));
    }
}

#[derive(Resource)]
struct StateTraceTimer(Timer);

/// State as of the last emitted span event.
#[derive(Default)]
struct LastEmitted {
    pos: Option<Vec2>,
    scene: Option<Scene>,
}

/// A move smaller than this (in world pixels) doesn't warrant a span event.
const MOVEMENT_EPSILON: f32 = 1.0;

fn state_changed(last: &LastEmitted, pos: Vec2, scene: Scene) -> bool {
    let moved = match last.pos {
        Some(prev) => prev.distance_squared(pos) > MOVEMENT_EPSILON * MOVEMENT_EPSILON,
        None => true,
    };
    moved || last.scene != Some(scene)
}

fn update_session_trace(
    time: Res<Time>,
    mut timer: ResMut<StateTraceTimer>,
    mut player_query: Query<(&Transform, &mut PlayerSessionTrace), With<Player>>,
    scene: Res<State<Scene>>,
    mut last: Local<LastEmitted>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let Some((transform, mut session_trace)) = player_query.iter_mut().next() else {
        return;
    };

    let pos = transform.translation.truncate();
    let current_scene = *scene.get();

    session_trace.span.set_attribute(KeyValue::new("player.x", f64::from(pos.x)));
    session_trace.span.set_attribute(KeyValue::new("player.y", f64::from(pos.y)));
    session_trace.span.set_attribute(KeyValue::new("game.scene", format!("{current_scene:?}")));

    if state_changed(&last, pos, current_scene) {
        session_trace.span.add_event("player.state_update", vec![
            KeyValue::new("player.x", f64::from(pos.x)),
            KeyValue::new("player.y", f64::from(pos.y)),
            KeyValue::new("game.scene", format!("{current_scene:?}")),
        ]);
        last.pos = Some(pos);
        last.scene = Some(current_scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_observation_always_emits() {
        let last = LastEmitted::default();
        assert!(state_changed(&last, Vec2::ZERO, Scene::TownOfEndgame));
    }

    #[test]
    fn idle_player_in_same_scene_does_not_emit() {
        let last = LastEmitted {
            pos: Some(Vec2::new(100.0, 100.0)),
            scene: Some(Scene::TownOfEndgame),
        };
        // Sub-pixel drift must not count as movement.
        assert!(!state_changed(&last, Vec2::new(100.4, 100.4), Scene::TownOfEndgame));
    }

    #[test]
    fn movement_emits() {
        let last = LastEmitted {
            pos: Some(Vec2::new(100.0, 100.0)),
            scene: Some(Scene::TownOfEndgame),
        };
        assert!(state_changed(&last, Vec2::new(148.0, 100.0), Scene::TownOfEndgame));
    }

    #[test]
    fn scene_change_emits_even_when_standing_still() {
        let last = LastEmitted {
            pos: Some(Vec2::new(100.0, 100.0)),
            scene: Some(Scene::TownOfEndgame),
        };
        assert!(state_changed(&last, Vec2::new(100.0, 100.0), Scene::TeamDisco));
    }
}
