use bevy::prelude::*;
use bevy::asset::AssetLoader;
use crate::game_state::GameState;
use crate::assets::GameAssets;
use crate::instrumentation::{GameTracer, GameMeter, ActiveDialogue, record_dialogue_line_event};
use opentelemetry::{KeyValue, Context as OtelContext, trace::{Tracer, Span as _}};
use serde::Deserialize;
use std::time::Instant;

#[derive(Deserialize, Asset, TypePath)]
pub struct DialogueData {
    pub speaker: String,
    pub portrait: Option<String>,
    pub lines: Vec<String>,
}

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DialogueData>()
            .init_asset_loader::<DialogueDataLoader>()
            .add_message::<StartDialogueEvent>()
            .add_systems(Update, handle_dialogue_events.run_if(in_state(GameState::Playing)))
            .add_systems(OnEnter(GameState::Dialogue), spawn_dialogue_ui)
            .add_systems(Update, (
                type_dialogue_text,
                advance_dialogue,
            ).run_if(in_state(GameState::Dialogue)))
            .add_systems(OnExit(GameState::Dialogue), despawn_dialogue_ui);
    }
}

#[derive(Message)]
pub struct StartDialogueEvent {
    pub speaker: String,
    pub portrait: Option<Handle<Image>>,
    pub lines: Vec<String>,
}

#[derive(Component)]
struct DialogueRoot;

#[derive(Component)]
struct DialogueTextNode;

#[derive(Component)]
struct SpeakerNameNode;

#[derive(Component)]
struct PortraitNode;

#[derive(Component)]
struct TypewriterEffect {
    full_text: String,
    current_index: usize,
    timer: Timer,
}

impl TypewriterEffect {
    fn new(text: String) -> Self {
        Self {
            full_text: text,
            current_index: 0,
            timer: Timer::from_seconds(0.03, TimerMode::Repeating),
        }
    }

    fn is_complete(&self) -> bool {
        self.current_index >= self.full_text.len()
    }

    fn skip_to_end(&mut self) {
        self.current_index = self.full_text.len();
    }
}

#[derive(Resource)]
pub struct DialogueQueue {
    speaker: String,
    portrait: Option<Handle<Image>>,
    lines: Vec<String>,
    current_line: usize,
}

impl DialogueQueue {
    fn new(speaker: String, portrait: Option<Handle<Image>>, lines: Vec<String>) -> Self {
        Self {
            speaker,
            portrait,
            lines,
            current_line: 0,
        }
    }

    fn current_text(&self) -> Option<String> {
        self.lines.get(self.current_line).cloned()
    }

    fn advance(&mut self) -> bool {
        self.current_line += 1;
        self.current_line < self.lines.len()
    }

    fn is_complete(&self) -> bool {
        self.current_line >= self.lines.len()
    }
}

fn spawn_dialogue_ui(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    dialogue_queue: Option<Res<DialogueQueue>>,
) {
    info!("🎨 spawn_dialogue_ui called");

    if dialogue_queue.is_none() {
        error!("❌ DialogueQueue resource not found!");
        return;
    }

    info!("✅ DialogueQueue found, spawning UI");

    let font = game_assets.dialogue_font.clone();

    commands.spawn((
        DialogueRoot,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            left: Val::Px(20.0),
            right: Val::Px(20.0),
            height: Val::Px(180.0),
            padding: UiRect::all(Val::Px(20.0)),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(15.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 0.95)),
        BorderColor::all(Color::WHITE),
    ))
    .with_children(|parent| {
        let portrait_handle = dialogue_queue
            .as_ref()
            .and_then(|queue| queue.portrait.clone());

        if let Some(portrait) = portrait_handle {
            parent.spawn((
                PortraitNode,
                ImageNode::new(portrait),
                Node {
                    width: Val::Px(128.0),
                    height: Val::Px(128.0),
                    ..default()
                },
            ));
        }

        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                flex_grow: 1.0,
                ..default()
            },
        ))
        .with_children(|text_parent| {
            let speaker_name = dialogue_queue
                .as_ref()
                .map(|q| q.speaker.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            text_parent.spawn((
                SpeakerNameNode,
                Text::new(speaker_name),
                TextFont {
                    font: font.clone(),
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.3)),
            ));

            let initial_text = dialogue_queue
                .as_ref()
                .and_then(|q| q.current_text())
                .unwrap_or_else(|| String::new());

            text_parent.spawn((
                DialogueTextNode,
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Left),
                TypewriterEffect::new(initial_text),
            ));
        });
    });
}

fn handle_dialogue_events(
    mut commands: Commands,
    mut events: MessageReader<StartDialogueEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    tracer: Res<GameTracer>,
) {
    for event in events.read() {
        info!("📖 Starting dialogue with: {} ({} lines)", event.speaker, event.lines.len());
        for (i, line) in event.lines.iter().enumerate() {
            info!("   Line {}: {}", i, line);
        }

        // Create dialogue session span
        // Note: This span will be a child of the current context (from NPC interaction)
        let context = OtelContext::current();
        let mut span = tracer.tracer()
            .start_with_context("dialogue.session", &context);

        span.set_attribute(KeyValue::new("dialogue.speaker", event.speaker.clone()));
        span.set_attribute(KeyValue::new("dialogue.total_lines", event.lines.len() as i64));

        // Add telemetry event for dialogue start
        span.add_event(
            "dialogue.resources_created",
            vec![
                KeyValue::new("queue.lines", event.lines.len() as i64),
                KeyValue::new("queue.speaker", event.speaker.clone()),
            ],
        );

        // Store active dialogue component as resource
        let active_dialogue = ActiveDialogue {
            span,
            start_time: Instant::now(),
            speaker: event.speaker.clone(),
            total_lines: event.lines.len(),
            chars_read: 0,
        };
        commands.insert_resource(active_dialogue);

        let queue = DialogueQueue::new(
            event.speaker.clone(),
            event.portrait.clone(),
            event.lines.clone(),
        );

        commands.insert_resource(queue);
        info!("🎮 Transitioning to Dialogue state");
        next_state.set(GameState::Dialogue);
    }
}

fn type_dialogue_text(
    time: Res<Time>,
    mut query: Query<(&mut Text, &mut TypewriterEffect), With<DialogueTextNode>>,
    mut active_dialogue: Option<ResMut<ActiveDialogue>>,
    dialogue_queue: Option<Res<DialogueQueue>>,
    meter: Res<GameMeter>,
) {
    for (mut text, mut typewriter) in &mut query {
        let was_complete = typewriter.is_complete();

        if was_complete {
            continue;
        }

        typewriter.timer.tick(time.delta());

        if typewriter.timer.just_finished() {
            if let Some(next_char) = typewriter.full_text.chars().nth(typewriter.current_index) {
                text.push(next_char);
                typewriter.current_index += 1;

                // Track characters read
                if let Some(ref mut dialogue) = active_dialogue {
                    dialogue.chars_read += 1;
                }
            }
        }

        // Record event when line completes
        if !was_complete && typewriter.is_complete() {
            if let (Some(dialogue), Some(queue)) = (&mut active_dialogue, &dialogue_queue) {
                record_dialogue_line_event(
                    &mut dialogue.span,
                    &typewriter.full_text,
                    queue.current_line,
                );

                // Record line counter metric
                meter.dialogue_lines_read.add(1, &[
                    KeyValue::new("speaker", dialogue.speaker.clone())
                ]);

                info!("📝 Dialogue line {} complete: {} chars",
                    queue.current_line,
                    typewriter.full_text.len());
            }
        }
    }
}

fn advance_dialogue(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut dialogue_queue: Option<ResMut<DialogueQueue>>,
    mut typewriter_query: Query<(&mut Text, &mut TypewriterEffect), With<DialogueTextNode>>,
) {
    if !keyboard.just_pressed(KeyCode::Space) && !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }

    if let Ok((mut text, mut typewriter)) = typewriter_query.single_mut() {
        if !typewriter.is_complete() {
            **text = typewriter.full_text.clone();
            typewriter.skip_to_end();
            return;
        }
    }

    if let Some(ref mut queue) = dialogue_queue {
        if queue.advance() {
            if let Some(next_text) = queue.current_text() {
                if let Ok((mut text, mut typewriter)) = typewriter_query.single_mut() {
                    **text = String::new();
                    *typewriter = TypewriterEffect::new(next_text);
                }
            }
        } else {
            info!("Dialogue sequence complete");
            next_state.set(GameState::Playing);
        }
    } else {
        next_state.set(GameState::Playing);
    }
}

fn despawn_dialogue_ui(
    mut commands: Commands,
    dialogue_root: Query<Entity, With<DialogueRoot>>,
    active_dialogue: Option<ResMut<ActiveDialogue>>,
    meter: Res<GameMeter>,
) {
    for entity in &dialogue_root {
        commands.entity(entity).despawn();
    }

    // Finalize dialogue session span and record metrics
    if let Some(mut dialogue) = active_dialogue {
        let duration_secs = dialogue.start_time.elapsed().as_secs_f64();
        let chars_read = dialogue.chars_read;
        let speaker = dialogue.speaker.clone();

        // Calculate reading speed (chars/second)
        let reading_speed = if duration_secs > 0.0 {
            chars_read as f64 / duration_secs
        } else {
            0.0
        };

        // Add final attributes to span
        dialogue.span.set_attribute(KeyValue::new("dialogue.chars_read", chars_read as i64));
        dialogue.span.set_attribute(KeyValue::new("dialogue.duration_secs", duration_secs));
        dialogue.span.set_attribute(KeyValue::new("dialogue.reading_speed", reading_speed));

        // Record reading speed metric
        meter.dialogue_reading_speed.record(
            reading_speed,
            &[KeyValue::new("speaker", speaker.clone())]
        );

        info!("📊 Dialogue session complete: {} chars in {:.2}s ({:.1} chars/sec)",
            chars_read,
            duration_secs,
            reading_speed);

        // Add telemetry event for resource cleanup
        dialogue.span.add_event(
            "dialogue.resources_removed",
            vec![
                KeyValue::new("cleanup.type", "normal"),
                KeyValue::new("dialogue.completed", true),
            ],
        );

        // Span ends when dropped (but we'll explicitly end it for clarity)
        dialogue.span.end();
        commands.remove_resource::<ActiveDialogue>();
    }

    commands.remove_resource::<DialogueQueue>();
    info!("Dialogue UI despawned");
}

#[derive(Default)]
struct DialogueDataLoader;

impl AssetLoader for DialogueDataLoader {
    type Asset = DialogueData;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // Enhanced error context with file size and path info
        let dialogue_data: DialogueData = serde_json::from_slice(&bytes)
            .map_err(|e| {
                let error_msg = format!(
                    "Failed to parse dialogue JSON (file: {}, size: {} bytes): {}",
                    load_context.path().display(),
                    bytes.len(),
                    e
                );
                error!("{}", error_msg);
                std::io::Error::new(std::io::ErrorKind::InvalidData, error_msg)
            })?;
        Ok(dialogue_data)
    }

    fn extensions(&self) -> &[&str] {
        &["dialogue.json"]
    }
}
