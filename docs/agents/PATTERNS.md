# PATTERNS.md - Reusable Knowledge

**Last Updated**: 2025-11-09 by Claude

## 🎨 Bevy 0.17 Patterns

### State Management Pattern

```rust
// Use States for major game phases
#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum GameState {
    Loading,
    Playing,
    Dialogue,
}

// Use SubStates for scene variations
#[derive(SubStates, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Playing)]
enum Scene {
    TownOfEndgame,
    TeamMarathon,
}

// Systems run conditionally with in_state()
app.add_systems(Update, player_movement.run_if(in_state(GameState::Playing)))
```

### Event-Driven Dialogue

```rust
// Define events for cross-system communication
#[derive(Event)]
enum StartDialogueEvent {
    Message(String),      // Single line
    Conversation(Handle<DialogueData>),  // Full dialogue tree
}

// Sender system
commands.trigger(StartDialogueEvent::Message("Hello!".to_string()));

// Receiver system
fn start_dialogue(
    trigger: Trigger<StartDialogueEvent>,
    // ... other params
) {
    // Handle event
}
```

### Proximity Detection Pattern

```rust
// Use marker components for transient state
#[derive(Component)]
struct InRange;

// Detection system
fn detect_proximity(
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    npc_query: Query<(Entity, &Transform, &InteractionRadius), With<Npc>>,
) {
    for (npc_entity, npc_transform, radius) in &npc_query {
        let distance = player_transform.translation.distance(npc_transform.translation);

        if distance <= radius.0 {
            commands.entity(npc_entity).insert(InRange);
        } else {
            commands.entity(npc_entity).remove::<InRange>();
        }
    }
}

// Action system - only affects entities with marker
fn interaction_system(
    npc_query: Query<&DialogueFile, (With<Npc>, With<InRange>)>,
) {
    // Only processes NPCs in range
}
```

## 🗺️ Asset Management

### Custom Asset Loaders

```rust
// Define asset type
#[derive(Asset, TypePath, Serialize, Deserialize)]
struct DialogueData {
    character: String,
    lines: Vec<String>,
}

// Implement AssetLoader
#[derive(Default)]
struct DialogueAssetLoader;

impl AssetLoader for DialogueAssetLoader {
    type Asset = DialogueData;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let data = serde_json::from_slice(&bytes)?;
        Ok(data)
    }

    fn extensions(&self) -> &[&str] {
        &["dialogue.json"]
    }
}

// Register in app
app.register_asset_loader(DialogueAssetLoader)
    .init_asset::<DialogueData>();
```

## 🎮 Game-Specific Patterns

### Camera Bounds with Safety

```rust
// Always check map size before constraining camera
let map_width = tilemap_size.x as f32 * tile_size.x;
let map_height = tilemap_size.y as f32 * tile_size.y;

// Prevent panic when map is smaller than viewport
let half_width = (WINDOW_WIDTH / 2.0).min(map_width / 2.0);
let half_height = (WINDOW_HEIGHT / 2.0).min(map_height / 2.0);

let clamped_x = player_x.clamp(half_width, map_width - half_width);
let clamped_y = player_y.clamp(half_height, map_height - half_height);
```

### Typewriter Effect with Multi-Stage Input

```rust
// State tracking for typewriter
#[derive(Component)]
struct TypewriterState {
    current_char: usize,
    total_chars: usize,
    timer: Timer,
}

// Two-stage advancement:
// 1. Space while typing -> complete current line
// 2. Space when complete -> advance to next line
fn advance_dialogue(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut typewriter_query: Query<&mut TypewriterState>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        if typewriter.current_char < typewriter.total_chars {
            // Stage 1: Complete typing
            typewriter.current_char = typewriter.total_chars;
        } else {
            // Stage 2: Next line
            advance_to_next_line();
        }
    }
}
```

## 🛠️ Development Patterns

### Plugin Organization

```rust
// One plugin per feature
pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
           .add_systems(Update, (
               player_movement,
               player_animation,
           ).run_if(in_state(GameState::Playing)));
    }
}

// Keep related functionality together
// - Component definitions
// - Systems
// - Events
// All in the same file (e.g., player.rs)
```

### Error Handling Strategy

```rust
// Use anyhow::Result for fallible operations
use anyhow::{Context, Result};

fn load_map_data(path: &str) -> Result<MapData> {
    let content = std::fs::read_to_string(path)
        .context(format!("Failed to read map file: {}", path))?;

    let map_data = serde_json::from_str(&content)
        .context("Failed to parse map JSON")?;

    Ok(map_data)
}

// Propagate errors with ? instead of unwrap()
// Add .context() for debugging breadcrumbs
```

## 📝 Data Format Patterns

### Clean Map Data Format

```rust
// Prefer clean, simple structures
{
    "name": "Town of Endgame",
    "width": 25,
    "height": 25,
    "layers": [
        {
            "name": "ground",
            "tiles": [[1, 1, 2], [1, 3, 3]]
        }
    ],
    "npcs": [
        {
            "name": "Nyaanager Evie",
            "x": 12, "y": 10,
            "sprite": "evie.png",
            "dialogue": "evie_intro.dialogue.json"
        }
    ]
}

// Avoid nested RPGMaker complexity
// Keep data structures flat and obvious
```

## 🎯 Best Practices Discovered

1. **State-driven systems**: Use Bevy states instead of bool flags
2. **Marker components**: Lightweight state tracking (InRange, Interactable)
3. **Event-driven communication**: Decouple systems with events
4. **Asset handles**: Load once, reference by handle
5. **Anyhow for errors**: Never unwrap(), always propagate with ?
6. **Run conditions**: Use `in_state()` to control system execution
7. **Component bundles**: Let Bevy auto-insert required components

## 🐛 Common Pitfalls

1. **Camera Z-fighting**: Always use 999.9 for 2D camera Z
2. **Blurry sprites**: Must use `ImagePlugin::default_nearest()`
3. **Map bounds panic**: Always validate map size vs viewport size
4. **State transitions**: Must manually transition from Loading to Playing
5. **Text wrapping**: Need manual implementation, no built-in solution in Bevy 0.17

---
*Add new patterns as they emerge. Document both successes and failures.*
