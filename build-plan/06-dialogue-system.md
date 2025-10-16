# Build Plan 06: Dialogue System with Typewriter Effect

## Objective

Implement a complete dialogue system with character portraits, typewriter text effect, and proper UI layout. This is the core mechanic for teaching SRE concepts through NPC conversations.

## Context

The dialogue system is central to the SRE Game experience:
- **Presentation**: Character portrait + name + text in a box at bottom of screen
- **Typewriter effect**: Text appears one character at a time
- **Player interaction**: Space/Enter advances dialogue, Escape exits conversation
- **State management**: Transitions between Playing and Dialogue states

**RPGMaker MZ Dialogue Format**:
```json
{
  "speaker": "Nyaanager Evie",
  "portrait": "Nature",
  "portrait_index": 2,
  "lines": [
    "I do my best to protect them but the pressure from Mahogany Row is getting to me.",
    "We had an incident last night but I told the team to take the day off."
  ]
}
```

## Prerequisites

- Completed: **01-project-setup.md** through **05-tilemap-rendering.md**
- GameState includes Dialogue state
- Player movement freezes in Dialogue state (already handled via `run_if(in_state(GameState::Playing))`)

## Tasks

### 1. Create dialogue.rs Module

Create `src/dialogue.rs`:

```rust
use bevy::prelude::*;
use crate::game_state::GameState;

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<StartDialogueEvent>()
            .add_systems(OnEnter(GameState::Dialogue), spawn_dialogue_ui)
            .add_systems(Update, (
                handle_dialogue_events,
                type_dialogue_text,
                advance_dialogue,
            ).run_if(in_state(GameState::Dialogue)))
            .add_systems(OnExit(GameState::Dialogue), despawn_dialogue_ui);
    }
}

/// Event to trigger a dialogue sequence
#[derive(Event)]
pub struct StartDialogueEvent {
    pub speaker: String,
    pub portrait: Option<Handle<Image>>,
    pub lines: Vec<String>,
}

/// Marker for the dialogue UI root
#[derive(Component)]
struct DialogueRoot;

/// Marker for the text node that shows dialogue
#[derive(Component)]
struct DialogueTextNode;

/// Marker for the speaker name node
#[derive(Component)]
struct SpeakerNameNode;

/// Marker for the portrait image
#[derive(Component)]
struct PortraitNode;

/// Component tracking typewriter effect progress
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
            timer: Timer::from_seconds(0.03, TimerMode::Repeating), // 30ms per character
        }
    }

    fn is_complete(&self) -> bool {
        self.current_index >= self.full_text.len()
    }

    fn skip_to_end(&mut self) {
        self.current_index = self.full_text.len();
    }
}

/// Resource storing the current dialogue queue
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
    asset_server: Res<AssetServer>,
    dialogue_queue: Option<Res<DialogueQueue>>,
) {
    info!("Spawning dialogue UI");

    let font = asset_server.load("fonts/dialogue.ttf");

    // Main dialogue box container
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
        BorderColor(Color::WHITE),
        BorderRadius::BorderRadius::all(Val::Px(10.0)),
    ))
    .with_children(|parent| {
        // Portrait section (left side)
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

        // Text section (right side)
        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                flex_grow: 1.0,
                ..default()
            },
        ))
        .with_children(|text_parent| {
            // Speaker name
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
                TextColor(Color::srgb(1.0, 0.85, 0.3)), // Golden color for names
            ));

            // Dialogue text
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
                TextLayout::new_with_justify(JustifyText::Left),
                TypewriterEffect::new(initial_text),
            ));
        });
    });
}

fn handle_dialogue_events(
    mut commands: Commands,
    mut events: EventReader<StartDialogueEvent>,
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

    // Check if typewriter is still typing
    if let Ok((_, mut typewriter)) = typewriter_query.get_single_mut() {
        if !typewriter.is_complete() {
            // Skip to end of current text
            if let Ok((mut text, _)) = typewriter_query.get_single_mut() {
                **text = typewriter.full_text.clone();
                typewriter.skip_to_end();
            }
            return;
        }
    }

    // Typewriter complete, advance to next line or exit
    if let Some(ref mut queue) = dialogue_queue {
        if queue.advance() {
            // More lines to show
            if let Some(next_text) = queue.current_text() {
                if let Ok((mut text, mut typewriter)) = typewriter_query.get_single_mut() {
                    **text = String::new();
                    *typewriter = TypewriterEffect::new(next_text);
                }
            }
        } else {
            // Dialogue complete
            info!("Dialogue sequence complete");
            next_state.set(GameState::Playing);
        }
    } else {
        // No queue, exit dialogue
        next_state.set(GameState::Playing);
    }
}

fn despawn_dialogue_ui(
    mut commands: Commands,
    dialogue_root: Query<Entity, With<DialogueRoot>>,
) {
    for entity in &dialogue_root {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<DialogueQueue>();
    info!("Dialogue UI despawned");
}
```

**Key Design Decisions**:
- **Event-driven**: `StartDialogueEvent` triggers dialogue from anywhere
- **Typewriter timing**: 30ms per character (adjustable)
- **Two-stage advance**: First press skips typing, second press advances line
- **Auto-cleanup**: UI despawns on `OnExit(GameState::Dialogue)`
- **Portrait optional**: Supports dialogues without character faces

### 2. Update main.rs to Include Dialogue Plugin

Modify `src/main.rs`:

```rust
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
                        resolution: (1920.0, 1080.0).into(),
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

// TEMPORARY: Test dialogue system with D key
fn test_dialogue_trigger(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dialogue_events: EventWriter<StartDialogueEvent>,
    asset_server: Res<AssetServer>,
) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        info!("Triggering test dialogue");

        // Create test dialogue
        let portrait = asset_server.load("textures/portraits/Nature.png");

        dialogue_events.send(StartDialogueEvent {
            speaker: "Nyaanager Evie".to_string(),
            portrait: Some(portrait),
            lines: vec![
                "I do my best to protect them but the pressure from Mahogany Row is getting to me.".to_string(),
                "We had an incident last night but I told the team to take the day off.".to_string(),
                "They needed rest more than we needed post-mortem heroics.".to_string(),
            ],
        });
    }
}
```

### 3. Add Font Asset

The dialogue system needs a font file at `assets/fonts/dialogue.ttf`.

**Option A**: Use a free game font
Download a pixel/game font like:
- **Press Start 2P**: https://fonts.google.com/specimen/Press+Start+2P
- **Pixelify Sans**: https://fonts.google.com/specimen/Pixelify+Sans

Save as `assets/fonts/dialogue.ttf`.

**Option B**: Use system font temporarily
```bash
# Copy a system font for testing (Linux)
cp /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf assets/fonts/dialogue.ttf
```

### 4. Add Portrait Asset (Optional for Testing)

For testing with portraits:

```bash
# Copy a character portrait from original game
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/faces/Nature.png \
   assets/textures/portraits/Nature.png
```

Or test without portraits by removing the portrait parameter from `StartDialogueEvent`.

### 5. Test Dialogue System

Run the application:

```bash
cargo run
```

Expected behavior:
- Player can move around map
- Press `D` → Dialogue UI appears at bottom of screen
- Speaker name shows in golden color
- Text types out character by character
- Press `Space`/`Enter`:
  - If text still typing → skip to end of line
  - If text complete → advance to next line
- After last line → returns to Playing state
- UI disappears cleanly
- Player can move again

### 6. Verify Two-Stage Advance

Test the interaction flow:
1. Press `D` to start dialogue
2. Wait for text to finish typing
3. Press `Space` → advances to line 2
4. While line 2 is typing, press `Space` → text completes instantly
5. Press `Space` again → advances to line 3
6. After line 3 completes, press `Space` → exits dialogue

This two-stage advance is standard in visual novels.

## Success Criteria

- [ ] `src/dialogue.rs` created with DialoguePlugin
- [ ] Dialogue UI spawns at bottom of screen
- [ ] Speaker name displays correctly
- [ ] Typewriter effect works (one char at a time)
- [ ] Portrait displays (if provided)
- [ ] Space/Enter advances dialogue correctly
- [ ] Two-stage advance works (skip typing, then advance line)
- [ ] Dialogue exits cleanly after last line
- [ ] Player movement frozen during dialogue
- [ ] UI despawns completely on exit
- [ ] No compilation errors or warnings

## Dialogue UI Layout Reference

```
┌─────────────────────────────────────────────────────┐
│ [Portrait]  Speaker Name (Golden)                   │
│             Dialogue text appears here with         │
│             typewriter effect at 30ms per char...   │
└─────────────────────────────────────────────────────┘
```

Dimensions:
- Total height: 180px
- Padding: 20px all sides
- Portrait: 128x128px (if present)
- Font sizes: Name 28px, Text 24px

## Data Format for Future Integration

When connecting to NPC system (step 07), dialogues will be loaded from JSON:

```json
{
  "npc_id": "nyaanager_evie",
  "speaker": "Nyaanager Evie",
  "portrait": "Nature",
  "portrait_index": 2,
  "dialogue": [
    "I do my best to protect them but the pressure from Mahogany Row is getting to me.",
    "We had an incident last night but I told the team to take the day off."
  ]
}
```

This will be implemented in **09-content-port.md**.

## Known Issues / Future Improvements

- **Word wrapping**: Long lines may overflow (add max width to text node)
- **Sound effects**: No typing sound or advance sound
- **Portrait expressions**: No support for changing expressions mid-dialogue
- **Multiple speakers**: One dialogue = one speaker (could add speaker changes)
- **Choice system**: No branching dialogues (linear only for MVP)

## Advanced Features (Optional)

### Add Typing Sound Effect

```rust
#[derive(Resource)]
struct TypingSoundTimer(Timer);

fn play_typing_sound(
    time: Res<Time>,
    mut timer: ResMut<TypingSoundTimer>,
    typewriter_query: Query<&TypewriterEffect, With<DialogueTextNode>>,
    // Add audio system when available
) {
    if let Ok(typewriter) = typewriter_query.get_single() {
        if !typewriter.is_complete() {
            timer.0.tick(time.delta());
            if timer.0.just_finished() {
                // Play blip sound
            }
        }
    }
}
```

### Support for Rich Text

Add BBCode-style formatting support:
- `<b>bold</b>`
- `<i>italic</i>`
- `<color=#ff0000>red text</color>`

This can be added later using Bevy's `TextSection` system.

## Next Steps

After completing this task:
1. **07-npc-interactions.md**: NPCs will trigger `StartDialogueEvent` when player interacts
2. **09-content-port.md**: Load actual dialogue content from JSON files
3. Remove `test_dialogue_trigger` system after step 07 is complete

## Notes for Implementation

- Dialogue box uses absolute positioning to stay at bottom of screen
- Portrait is optional - layout adjusts if portrait is None
- TypewriterEffect timer should be fast enough to feel responsive but slow enough to read
- The two-stage advance pattern (skip → advance) is industry standard for visual novels
- Escape key exits dialogue immediately (handled in game_state.rs)

## Reference Files

- Original dialogue data: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Map*.json` (event commands)
- Character portraits: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/faces/`
- RPGMaker dialogue commands: Code 101 (face), Code 401 (text)
