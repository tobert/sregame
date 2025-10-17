use bevy::prelude::*;
use bevy::asset::AssetLoader;
use crate::game_state::GameState;
use crate::assets::GameAssets;
use serde::Deserialize;

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
struct DialogueQueue {
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
    info!("Spawning dialogue UI");

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
) {
    for event in events.read() {
        info!("Starting dialogue with: {}", event.speaker);

        let queue = DialogueQueue::new(
            event.speaker.clone(),
            event.portrait.clone(),
            event.lines.clone(),
        );

        commands.insert_resource(queue);
        next_state.set(GameState::Dialogue);
    }
}

fn type_dialogue_text(
    time: Res<Time>,
    mut query: Query<(&mut Text, &mut TypewriterEffect), With<DialogueTextNode>>,
) {
    for (mut text, mut typewriter) in &mut query {
        if typewriter.is_complete() {
            continue;
        }

        typewriter.timer.tick(time.delta());

        if typewriter.timer.just_finished() {
            if let Some(next_char) = typewriter.full_text.chars().nth(typewriter.current_index) {
                text.push(next_char);
                typewriter.current_index += 1;
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
) {
    for entity in &dialogue_root {
        commands.entity(entity).despawn();
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
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let dialogue_data: DialogueData = serde_json::from_slice(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(dialogue_data)
    }

    fn extensions(&self) -> &[&str] {
        &["dialogue.json"]
    }
}
