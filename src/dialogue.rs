use bevy::prelude::*;
use crate::game_state::Mode;
use crate::assets::GameAssets;
use crate::instrumentation::{GameTracer, GameMeter, ActiveDialogue, record_dialogue_line_event};
use opentelemetry::{KeyValue, Context as OtelContext, trace::{Tracer, Span as _}};
use web_time::Instant;

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StartDialogueEvent>()
            .add_systems(Update, handle_dialogue_events.run_if(in_state(Mode::Exploring)))
            .add_systems(OnEnter(Mode::Dialogue), spawn_dialogue_ui)
            .add_systems(Update, (
                type_dialogue_text,
                advance_dialogue,
            ).run_if(in_state(Mode::Dialogue)))
            .add_systems(OnExit(Mode::Dialogue), despawn_dialogue_ui);
    }
}

/// One message box: its own speaker and portrait. A plain NPC conversation
/// is a run of segments sharing one speaker; a scripted scene (the retro
/// retrospective) switches speaker/portrait between segments.
#[derive(Clone)]
pub struct DialogueSegment {
    pub speaker: String,
    /// Asset path like "textures/portraits/Nature.png"; empty = no portrait.
    /// Paths (not handles) so senders don't need an AssetServer - the UI
    /// resolves them when each segment is shown.
    pub portrait_path: String,
    /// Which cell of the face sheet to crop - see FACE_SHEET_* below.
    pub portrait_face_index: u32,
    pub text: String,
}

#[derive(Message)]
pub struct StartDialogueEvent {
    pub segments: Vec<DialogueSegment>,
}

/// RPGMaker MZ face sheets are always a 4-column x 2-row grid of 144x144px
/// cells (`ImageManager.faceWidth`/`faceHeight` in rmmz_managers.js and
/// `Window_Base.prototype.drawFace` in rmmz_windows.js are hardcoded to this
/// regardless of a given sheet's actual pixel dimensions - some of ours are
/// taller than 288px, e.g. assets/textures/portraits/casey.png, with the
/// extra rows simply unused), so `faceIndex` 0-7 always maps into this one
/// fixed grid across every portrait file.
const FACE_SHEET_CELL_SIZE: UVec2 = UVec2::new(144, 144);
const FACE_SHEET_COLUMNS: u32 = 4;
const FACE_SHEET_ROWS: u32 = 2;

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
    segments: Vec<DialogueSegment>,
    current: usize,
    /// One shared face-sheet atlas layout for the whole conversation
    /// (created by spawn_dialogue_ui) so segment changes don't mint a new
    /// layout asset per box.
    face_layout: Option<Handle<TextureAtlasLayout>>,
}

impl DialogueQueue {
    fn new(segments: Vec<DialogueSegment>) -> Self {
        Self { segments, current: 0, face_layout: None }
    }

    fn current_segment(&self) -> Option<&DialogueSegment> {
        self.segments.get(self.current)
    }

    fn advance(&mut self) -> bool {
        self.current += 1;
        self.current < self.segments.len()
    }
}

fn spawn_dialogue_ui(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    asset_server: Res<AssetServer>,
    dialogue_queue: Option<ResMut<DialogueQueue>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let Some(mut queue) = dialogue_queue else {
        error!("❌ DialogueQueue resource not found!");
        return;
    };

    let font = game_assets.dialogue_font.clone();

    // Face sheets are a grid of cells, not one portrait each (see
    // FACE_SHEET_* docs above) - render only the segment's own cell via a
    // texture atlas, the same idiom npc.rs::spawn_npc uses for walk-sprite
    // sheets. One layout serves every segment of the conversation.
    let atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        FACE_SHEET_CELL_SIZE,
        FACE_SHEET_COLUMNS,
        FACE_SHEET_ROWS,
        None,
        None,
    ));
    queue.face_layout = Some(atlas_layout.clone());

    let first = queue.current_segment().cloned().unwrap_or(DialogueSegment {
        speaker: "Unknown".into(),
        portrait_path: String::new(),
        portrait_face_index: 0,
        text: String::new(),
    });

    // Presentation-scale layout: the box claims the bottom third of the
    // window so the text can be read from the back of a conference room.
    commands.spawn((
        DialogueRoot,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            height: Val::Percent(33.3),
            padding: UiRect::all(Val::Px(24.0)),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(24.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 0.95)),
        BorderColor::all(Color::WHITE),
    ))
    .with_children(|parent| {
        // The portrait node always exists so later segments can swap faces
        // in (or hide it) without re-spawning UI - Display::None when the
        // current segment has no portrait. Square aspect + full height so
        // it scales with the box instead of a hardcoded pixel size.
        let (image_node, display) = portrait_for_segment(&first, &asset_server, &atlas_layout);
        parent.spawn((
            PortraitNode,
            image_node,
            Node {
                height: Val::Percent(100.0),
                aspect_ratio: Some(1.0),
                display,
                ..default()
            },
        ));

        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                flex_grow: 1.0,
                ..default()
            },
        ))
        .with_children(|text_parent| {
            text_parent.spawn((
                SpeakerNameNode,
                Text::new(first.speaker.clone()),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(52.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.3)),
            ));

            text_parent.spawn((
                DialogueTextNode,
                Text::new(""),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(46.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::justify(Justify::Left),
                TypewriterEffect::new(first.text.clone()),
            ));
        });
    });
}

/// Builds the portrait ImageNode (and node display state) for a segment.
/// Empty portrait path = hidden node.
fn portrait_for_segment(
    segment: &DialogueSegment,
    asset_server: &AssetServer,
    atlas_layout: &Handle<TextureAtlasLayout>,
) -> (ImageNode, Display) {
    if segment.portrait_path.is_empty() {
        return (ImageNode::default(), Display::None);
    }

    #[cfg(debug_assertions)]
    {
        let max_index = FACE_SHEET_COLUMNS * FACE_SHEET_ROWS - 1;
        if segment.portrait_face_index > max_index {
            error!(
                "❌ Portrait face_index {} exceeds face sheet grid bounds (max {})",
                segment.portrait_face_index, max_index
            );
        }
    }

    (
        ImageNode::from_atlas_image(
            asset_server.load(&segment.portrait_path),
            TextureAtlas {
                layout: atlas_layout.clone(),
                index: segment.portrait_face_index as usize,
            },
        ),
        Display::Flex,
    )
}

fn handle_dialogue_events(
    mut commands: Commands,
    mut events: MessageReader<StartDialogueEvent>,
    mut next_mode: ResMut<NextState<Mode>>,
    tracer: Option<Res<GameTracer>>,
) {
    for event in events.read() {
        if event.segments.is_empty() {
            warn!("StartDialogueEvent with no segments - ignoring");
            continue;
        }
        let first_speaker = event.segments[0].speaker.clone();
        info!("📖 Starting dialogue: {} ({} segments)", first_speaker, event.segments.len());

        // Create dialogue session span (if telemetry is enabled)
        if let Some(tracer) = tracer.as_ref() {
            // Note: This span will be a child of the current context (from NPC interaction)
            let context = OtelContext::current();
            let mut span = tracer.tracer()
                .start_with_context("dialogue.session", &context);

            span.set_attribute(KeyValue::new("dialogue.speaker", first_speaker.clone()));
            span.set_attribute(KeyValue::new("dialogue.total_lines", event.segments.len() as i64));

            // Add telemetry event for dialogue start
            span.add_event(
                "dialogue.resources_created",
                vec![
                    KeyValue::new("queue.lines", event.segments.len() as i64),
                    KeyValue::new("queue.speaker", first_speaker.clone()),
                ],
            );

            // Store active dialogue component as resource
            let active_dialogue = ActiveDialogue {
                span,
                start_time: Instant::now(),
                speaker: first_speaker,
                chars_read: 0,
            };
            commands.insert_resource(active_dialogue);
        }

        commands.insert_resource(DialogueQueue::new(event.segments.clone()));
        info!("🎮 Transitioning to Dialogue mode");
        next_mode.set(Mode::Dialogue);
    }
}

fn type_dialogue_text(
    time: Res<Time>,
    mut query: Query<(&mut Text, &mut TypewriterEffect), With<DialogueTextNode>>,
    mut active_dialogue: Option<ResMut<ActiveDialogue>>,
    dialogue_queue: Option<Res<DialogueQueue>>,
    meter: Option<Res<GameMeter>>,
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
                    queue.current,
                );

                if let Some(ref meter) = meter {
                    let speaker = queue
                        .current_segment()
                        .map(|s| s.speaker.clone())
                        .unwrap_or_else(|| dialogue.speaker.clone());
                    meter.dialogue_lines_read.add(1, &[
                        KeyValue::new("speaker", speaker)
                    ]);
                }

                info!("📝 Dialogue segment {} complete: {} chars",
                    queue.current,
                    typewriter.full_text.len());
            }
        }
    }
}

fn advance_dialogue(
    keyboard: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut next_mode: ResMut<NextState<Mode>>,
    mut dialogue_queue: Option<ResMut<DialogueQueue>>,
    mut typewriter_query: Query<(&mut Text, &mut TypewriterEffect), With<DialogueTextNode>>,
    mut speaker_query: Query<&mut Text, (With<SpeakerNameNode>, Without<DialogueTextNode>)>,
    mut portrait_query: Query<(&mut ImageNode, &mut Node), With<PortraitNode>>,
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
            let Some(segment) = queue.current_segment().cloned() else {
                return;
            };
            if let Ok((mut text, mut typewriter)) = typewriter_query.single_mut() {
                **text = String::new();
                *typewriter = TypewriterEffect::new(segment.text.clone());
            }
            // Each segment carries its own speaker/portrait - a scripted
            // scene switches faces mid-conversation.
            if let Ok(mut speaker_text) = speaker_query.single_mut() {
                **speaker_text = segment.speaker.clone();
            }
            if let (Ok((mut image, mut node)), Some(layout)) =
                (portrait_query.single_mut(), queue.face_layout.as_ref())
            {
                let (new_image, display) = portrait_for_segment(&segment, &asset_server, layout);
                *image = new_image;
                node.display = display;
            }
        } else {
            info!("Dialogue sequence complete");
            next_mode.set(Mode::Exploring);
        }
    } else {
        next_mode.set(Mode::Exploring);
    }
}

fn despawn_dialogue_ui(
    mut commands: Commands,
    dialogue_root: Query<Entity, With<DialogueRoot>>,
    active_dialogue: Option<ResMut<ActiveDialogue>>,
    meter: Option<Res<GameMeter>>,
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

        if let Some(ref meter) = meter {
            meter.dialogue_reading_speed.record(
                reading_speed,
                &[KeyValue::new("speaker", speaker.clone())]
            );
        }

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
