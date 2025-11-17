use bevy::prelude::*;
use crate::game_state::GameState;
use crate::player::Player;
use crate::dialogue::StartDialogueEvent;
use crate::assets::GameAssets;
use crate::instrumentation::{GameTracer, GameMeter, PlayerSessionTrace, start_npc_interaction_span};
use opentelemetry::{KeyValue, trace::{Span as _, Tracer}};

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            check_npc_proximity,
            handle_interaction_input,
        ).chain().run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component)]
pub struct Npc {
    pub name: String,
    pub sprite_facing: NpcFacing,
}

#[derive(Clone, Copy)]
pub enum NpcFacing {
    Down = 0,
    Left = 1,
    Right = 2,
    Up = 3,
}

#[derive(Component)]
pub struct NpcDialogue {
    pub speaker: String,
    pub portrait_path: String,
    pub lines: Vec<String>,
}

#[derive(Component)]
pub struct Interactable {
    pub radius: f32,
    pub prompt: String,
}

impl Default for Interactable {
    fn default() -> Self {
        Self {
            radius: 64.0,
            prompt: "Press E to talk".to_string(),
        }
    }
}

#[derive(Component)]
struct InRange;

#[derive(Component)]
struct InteractionPrompt;

pub fn spawn_npc(
    commands: &mut Commands,
    _game_assets: &GameAssets,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    position: Vec3,
    sprite_handle: Handle<Image>,
    npc_data: Npc,
    dialogue: NpcDialogue,
    tracer: Option<&GameTracer>,
) -> Entity {
    let texture = sprite_handle;

    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(48, 48),
        3,
        4,
        None,
        None,
    );
    let atlas_layout = texture_atlas_layouts.add(layout);

    let sprite_index = npc_data.sprite_facing as usize * 3 + 1;

    // Validate sprite index in debug mode
    #[cfg(debug_assertions)]
    {
        let max_index = 3 * 4 - 1; // 3x4 grid = indices 0-11
        if sprite_index > max_index {
            error!("❌ NPC sprite index {} exceeds grid bounds (max {})",
                sprite_index, max_index);
        }
    }

    // Add telemetry for NPC spawn
    if let Some(t) = tracer {
        let mut span = t.tracer().start("npc.spawned");
        span.set_attribute(KeyValue::new("npc.name", npc_data.name.clone()));
        span.set_attribute(KeyValue::new("npc.x", position.x as f64));
        span.set_attribute(KeyValue::new("npc.y", position.y as f64));
        span.set_attribute(KeyValue::new("npc.sprite_index", sprite_index as i64));
        span.end();
    }

    info!("👤 NPC spawned: {} at ({:.0}, {:.0})", npc_data.name, position.x, position.y);

    commands.spawn((
        npc_data,
        dialogue,
        Interactable::default(),
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: sprite_index,
            },
        ),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(2.0)),
    )).id()
}

fn check_npc_proximity(
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    npc_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, Without<InRange>)>,
    in_range_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, With<InRange>)>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();

    for (entity, npc_transform, interactable) in &npc_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if distance <= interactable.radius {
            commands.entity(entity).insert(InRange);
        }
    }

    for (entity, npc_transform, interactable) in &in_range_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if distance > interactable.radius {
            commands.entity(entity).remove::<InRange>();
        }
    }
}

fn handle_interaction_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    player_query: Query<(&Transform, &PlayerSessionTrace), With<Player>>,
    npc_query: Query<(&Transform, &NpcDialogue), (With<Npc>, With<InRange>)>,
    mut dialogue_events: MessageWriter<StartDialogueEvent>,
    asset_server: Res<AssetServer>,
    tracer: Res<GameTracer>,
    meter: Res<GameMeter>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    let Ok((player_transform, session_trace)) = player_query.single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();

    let mut closest_npc: Option<(&NpcDialogue, f32)> = None;

    for (npc_transform, dialogue) in &npc_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if let Some((_, closest_dist)) = closest_npc {
            if distance < closest_dist {
                closest_npc = Some((dialogue, distance));
            }
        } else {
            closest_npc = Some((dialogue, distance));
        }
    }

    if let Some((dialogue, distance)) = closest_npc {
        // Start NPC interaction span
        let mut span = start_npc_interaction_span(
            &tracer,
            session_trace,
            &dialogue.speaker,
            player_pos,
            distance,
        );

        // Record interaction metric
        meter.interactions_total.add(
            1,
            &[KeyValue::new("npc.name", dialogue.speaker.clone())]
        );

        info!("🤝 NPC interaction started: {} (distance: {:.1}px)", dialogue.speaker, distance);

        let portrait = asset_server.load(&dialogue.portrait_path);

        // Set this span as the current context for dialogue event processing
        let context = opentelemetry::Context::current_with_value(span.span_context().clone());
        let _guard = context.attach();

        dialogue_events.write(StartDialogueEvent {
            speaker: dialogue.speaker.clone(),
            portrait: Some(portrait),
            lines: dialogue.lines.clone(),
        });

        // Span ends here (dropped) - the interaction span is brief
        // Dialogue will have its own child span (created in handle_dialogue_events)
        drop(_guard);
        span.end();
    }
}
