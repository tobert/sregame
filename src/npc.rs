use bevy::prelude::*;
use crate::game_state::Mode;
use crate::player::Player;
use crate::dialogue::StartDialogueEvent;
use crate::assets::GameAssets;
use crate::instrumentation::{GameTracer, GameMeter, PlayerSessionTrace, start_npc_interaction_span};
use opentelemetry::{KeyValue, trace::{Span as _, Tracer}};

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Npc>()
            .register_type::<NpcFacing>()
            .register_type::<NpcDialogue>()
            .register_type::<Interactable>()
            .add_systems(Update, (
            check_npc_proximity,
            handle_interaction_input,
        ).chain().run_if(in_state(Mode::Exploring)));
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Npc {
    pub name: String,
    pub sprite_facing: NpcFacing,
    /// Character slot (0-7) within the sprite sheet - see character_sheet.rs.
    pub sprite_slot: u32,
}

#[derive(Clone, Copy, Reflect, Default)]
pub enum NpcFacing {
    #[default]
    Down = 0,
    Left = 1,
    Right = 2,
    Up = 3,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct NpcDialogue {
    pub speaker: String,
    pub portrait_path: String,
    /// Which cell of the `portrait_path` face sheet to crop and display (see
    /// `DialogueData::face_index` in map_data.rs and the atlas built in
    /// `dialogue.rs::spawn_dialogue_ui`).
    pub portrait_face_index: u32,
    pub lines: Vec<String>,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
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

    let atlas_layout = texture_atlas_layouts.add(crate::character_sheet::sheet_layout());

    let sprite_index = crate::character_sheet::atlas_index(
        npc_data.sprite_slot,
        npc_data.sprite_facing as u32,
        crate::character_sheet::STANDING_PATTERN,
    ) as usize;

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
        Transform::from_translation(position),
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
    player_query: Query<(&Transform, Option<&PlayerSessionTrace>), With<Player>>,
    npc_query: Query<(&Transform, &NpcDialogue), (With<Npc>, With<InRange>)>,
    mut dialogue_events: MessageWriter<StartDialogueEvent>,
    asset_server: Res<AssetServer>,
    tracer: Option<Res<GameTracer>>,
    meter: Option<Res<GameMeter>>,
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
        info!("🤝 NPC interaction started: {} (distance: {:.1}px)", dialogue.speaker, distance);

        // Telemetry: Start NPC interaction span (if available)
        let telemetry_guard = if let (Some(tracer), Some(meter), Some(session_trace)) = (&tracer, &meter, session_trace) {
            let span = start_npc_interaction_span(
                tracer,
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

            // Set this span as the current context for dialogue event processing
            let context = opentelemetry::Context::current_with_value(span.span_context().clone());
            let guard = context.attach();
            Some((span, guard))
        } else {
            None
        };

        let portrait = asset_server.load(&dialogue.portrait_path);

        dialogue_events.write(StartDialogueEvent {
            speaker: dialogue.speaker.clone(),
            portrait: Some(portrait),
            portrait_face_index: dialogue.portrait_face_index,
            lines: dialogue.lines.clone(),
        });

        // Clean up telemetry span if it was created
        if let Some((mut span, guard)) = telemetry_guard {
            drop(guard);
            span.end();
        }
    }
}
