# Build Plan 09: Content Port from RPGMaker MZ

## Objective

Port actual game content from the original RPGMaker MZ game to the Bevy implementation. This includes map data, NPC positions, dialogue text, and collision information from Team Marathon and Town of Endgame.

## Context

The original game data is stored in JSON files at `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/`:
- **Map002.json**: Town of Endgame (hub, 34x39 tiles)
- **Map004.json**: Team Marathon interior (24x21 tiles)
- **Tilesets.json**: Tileset metadata including collision flags
- **MapInfos.json**: Map hierarchy and names

**RPGMaker Data Structures**:
```json
{
  "width": 34,
  "height": 39,
  "data": [1, 2, 3, ...],      // Tile indices (width * height * 4 layers)
  "events": [                  // NPC and event positions
    {
      "id": 1,
      "x": 12,
      "y": 8,
      "pages": [{ "image": {...}, "list": [...] }]
    }
  ]
}
```

## Prerequisites

- Completed: **01-project-setup.md** through **08-asset-loading.md**
- All systems functional with test data
- Original RPGMaker game files available at `/home/atobey/src/endgame-of-sre-rpgmaker-mz/`

## Tasks

### 1. Create Data Structures for JSON Deserialization

Create `src/rpgmaker_data.rs`:

```rust
use serde::{Deserialize, Serialize};

/// RPGMaker MZ map data structure
#[derive(Debug, Deserialize)]
pub struct RpgMakerMap {
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    #[serde(rename = "tilesetId")]
    pub tileset_id: u32,
    pub data: Vec<u32>,
    pub events: Vec<Option<RpgMakerEvent>>,
}

/// RPGMaker event (NPC or trigger)
#[derive(Debug, Deserialize)]
pub struct RpgMakerEvent {
    pub id: u32,
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub pages: Vec<EventPage>,
}

/// Event page containing graphics and dialogue
#[derive(Debug, Deserialize)]
pub struct EventPage {
    pub image: EventImage,
    pub list: Vec<EventCommand>,
}

/// Event graphics (sprite)
#[derive(Debug, Deserialize)]
pub struct EventImage {
    #[serde(rename = "characterName")]
    pub character_name: String,
    #[serde(rename = "characterIndex")]
    pub character_index: u32,
    pub direction: u32,
}

/// Event command (dialogue, movement, etc.)
#[derive(Debug, Deserialize)]
pub struct EventCommand {
    pub code: u32,
    pub parameters: Vec<serde_json::Value>,
}

impl EventCommand {
    /// Check if this is a "Show Text" command (code 401)
    pub fn is_dialogue(&self) -> bool {
        self.code == 401
    }

    /// Check if this is a "Show Face" command (code 101)
    pub fn is_show_face(&self) -> bool {
        self.code == 101
    }

    /// Extract text from dialogue command
    pub fn as_text(&self) -> Option<String> {
        if self.is_dialogue() {
            self.parameters.first()?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Extract face data (portrait) from Show Face command
    pub fn as_face_data(&self) -> Option<(String, u32)> {
        if self.is_show_face() {
            let face_name = self.parameters.get(0)?.as_str()?.to_string();
            let face_index = self.parameters.get(1)?.as_u64()? as u32;
            Some((face_name, face_index))
        } else {
            None
        }
    }
}

/// Parse dialogue from event command list
pub fn extract_dialogue(commands: &[EventCommand]) -> Option<(String, String, Vec<String>)> {
    let mut speaker = String::from("Unknown");
    let mut portrait = String::new();
    let mut lines = Vec::new();

    for command in commands {
        if let Some((face_name, _face_index)) = command.as_face_data() {
            portrait = face_name;
        }

        if let Some(text) = command.as_text() {
            lines.push(text);
        }
    }

    if !lines.is_empty() {
        // Try to extract speaker name from event commands or use first line pattern
        // RPGMaker often doesn't store speaker separately
        Some((speaker, portrait, lines))
    } else {
        None
    }
}

/// Convert RPGMaker tile position to Bevy world position
pub fn tile_to_world(tile_x: u32, tile_y: u32, map_width: u32, map_height: u32) -> (f32, f32) {
    const TILE_SIZE: f32 = 48.0;

    let world_x = (tile_x as f32 - map_width as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;
    let world_y = (tile_y as f32 - map_height as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;

    (world_x, world_y)
}

/// Convert RPGMaker direction to NpcFacing
pub fn rpgmaker_direction_to_facing(direction: u32) -> crate::npc::NpcFacing {
    match direction {
        2 => crate::npc::NpcFacing::Down,
        4 => crate::npc::NpcFacing::Left,
        6 => crate::npc::NpcFacing::Right,
        8 => crate::npc::NpcFacing::Up,
        _ => crate::npc::NpcFacing::Down,
    }
}
```

**Key Functions**:
- `extract_dialogue()`: Parses command list to get speaker, portrait, and text
- `tile_to_world()`: Converts tile coordinates to Bevy world space
- `rpgmaker_direction_to_facing()`: Maps RPGMaker directions (2/4/6/8) to NpcFacing

### 2. Load Map Data from JSON

Add map loading function to `src/tilemap.rs`:

```rust
use crate::rpgmaker_data::{RpgMakerMap, tile_to_world};
use anyhow::{Context, Result};
use std::fs;

fn load_rpgmaker_map(map_file: &str) -> Result<RpgMakerMap> {
    let map_path = format!("/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/{}", map_file);
    let json_data = fs::read_to_string(&map_path)
        .context(format!("Failed to read map file: {}", map_file))?;

    let map: RpgMakerMap = serde_json::from_str(&json_data)
        .context("Failed to parse map JSON")?;

    Ok(map)
}

fn spawn_town_of_endgame(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
) {
    info!("Loading Town of Endgame from RPGMaker data");

    // Load actual map data
    let map = match load_rpgmaker_map("Map002.json") {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to load map: {:?}", e);
            return;
        }
    };

    info!("Loaded map: {} ({}x{})", map.display_name, map.width, map.height);

    const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 48.0, y: 48.0 };
    const GRID_SIZE: TilemapGridSize = TilemapGridSize { x: 48.0, y: 48.0 };

    let texture_handle = game_assets.town_tileset.clone();
    let map_size = TilemapSize { x: map.width, y: map.height };
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(map_size);

    // RPGMaker data is 4 layers interleaved: [layer0, layer1, layer2, layer3]
    // We'll use layer 0 (ground) for now
    let layer_size = (map.width * map.height) as usize;

    for y in 0..map.height {
        for x in 0..map.width {
            let tile_pos = TilePos { x, y };
            let index = (y * map.width + x) as usize;

            // Get tile from layer 0
            let rpgmaker_tile = map.data.get(index).copied().unwrap_or(0);

            // Convert RPGMaker tile ID to our tileset index
            // RPGMaker uses complex tile ID encoding - simplified here
            let texture_index = if rpgmaker_tile > 0 {
                rpgmaker_tile - 1 // Basic conversion
            } else {
                0
            };

            let tile_entity = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(texture_index),
                        ..default()
                    },
                    Map,
                ))
                .id();

            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    commands.entity(tilemap_entity).insert((
        TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: TilemapTexture::Single(texture_handle),
            tile_size: TILE_SIZE,
            transform: Transform::from_xyz(
                -(map.width as f32 * TILE_SIZE.x) / 2.0,
                -(map.height as f32 * TILE_SIZE.y) / 2.0,
                0.0,
            ),
            ..default()
        },
        Map,
    ));

    // Create collision map (simplified - mark edges as blocked)
    let mut collision_map = CollisionMap::new(map.width, map.height);
    // TODO: Load actual collision data from tileset properties

    commands.insert_resource(collision_map);

    // Set camera bounds
    if let Ok(mut camera_follow) = camera_query.get_single_mut() {
        let map_width_pixels = map.width as f32 * TILE_SIZE.x;
        let map_height_pixels = map.height as f32 * TILE_SIZE.y;

        camera_follow.bounds = Some(CameraBounds::from_map_size(
            map_width_pixels,
            map_height_pixels,
            960.0,
            540.0,
        ));
    }
}
```

### 3. Load NPCs from Map Events

Add NPC spawning from map data to `src/tilemap.rs`:

```rust
use crate::rpgmaker_data::{extract_dialogue, rpgmaker_direction_to_facing};
use crate::npc::{spawn_npc, Npc, NpcDialogue};

fn spawn_npcs_from_map(
    commands: &mut Commands,
    game_assets: &GameAssets,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    map: &RpgMakerMap,
) {
    info!("Spawning NPCs from map events");

    for event_opt in &map.events {
        let Some(event) = event_opt else { continue };

        // Skip events without graphics (triggers, not NPCs)
        if event.pages.is_empty() {
            continue;
        }

        let page = &event.pages[0];
        if page.image.character_name.is_empty() {
            continue;
        }

        // Extract dialogue
        let dialogue_data = extract_dialogue(&page.list);

        let (speaker, portrait, lines) = if let Some(data) = dialogue_data {
            data
        } else {
            // NPC without dialogue, skip for now
            continue;
        };

        // Convert position
        let (world_x, world_y) = tile_to_world(event.x, event.y, map.width, map.height);

        // Get sprite handle based on character name
        let sprite_handle = match page.image.character_name.as_str() {
            "Nature" => game_assets.npc_nature.clone(),
            "People1" => game_assets.npc_people1.clone(),
            _ => {
                warn!("Unknown character sprite: {}", page.image.character_name);
                continue;
            }
        };

        // Get portrait path
        let portrait_path = if !portrait.is_empty() {
            format!("textures/portraits/{}.png", portrait)
        } else {
            String::new()
        };

        spawn_npc(
            commands,
            game_assets,
            texture_atlas_layouts,
            Vec3::new(world_x, world_y, 1.0),
            sprite_handle,
            Npc {
                name: event.name.clone(),
                sprite_facing: rpgmaker_direction_to_facing(page.image.direction),
            },
            NpcDialogue {
                speaker,
                portrait_path,
                lines,
            },
        );

        info!("Spawned NPC: {} at ({}, {})", event.name, event.x, event.y);
    }
}

// Update spawn_town_of_endgame to call this:
fn spawn_town_of_endgame(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
) {
    // ... existing map loading code ...

    // Spawn NPCs from map events
    spawn_npcs_from_map(
        &mut commands,
        &game_assets,
        &mut texture_atlas_layouts,
        &map,
    );
}
```

### 4. Create Helper Script to Extract Dialogue

Create a helper tool to extract and preview dialogue (optional but useful):

Create `tools/extract_dialogue.py`:

```python
#!/usr/bin/env python3
import json
import sys

def extract_dialogue_from_map(map_file):
    with open(map_file, 'r', encoding='utf-8') as f:
        map_data = json.load(f)

    print(f"Map: {map_data['displayName']}")
    print(f"Size: {map_data['width']}x{map_data['height']}\n")

    for event in map_data['events']:
        if event is None:
            continue

        if not event['pages']:
            continue

        page = event['pages'][0]

        # Extract dialogue
        speaker = "Unknown"
        portrait = ""
        lines = []

        for cmd in page['list']:
            if cmd['code'] == 101:  # Show Face
                portrait = cmd['parameters'][0]
            elif cmd['code'] == 401:  # Show Text
                lines.append(cmd['parameters'][0])

        if lines:
            print(f"NPC: {event['name']} at ({event['x']}, {event['y']})")
            print(f"  Portrait: {portrait}")
            print(f"  Dialogue:")
            for line in lines:
                print(f"    - {line}")
            print()

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: extract_dialogue.py <map_json_file>")
        sys.exit(1)

    extract_dialogue_from_map(sys.argv[1])
```

Usage:
```bash
chmod +x tools/extract_dialogue.py
./tools/extract_dialogue.py /home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Map004.json
```

This helps verify dialogue extraction before implementing in Rust.

### 5. Add Module Declaration

Update `src/main.rs`:

```rust
mod rpgmaker_data;

// ... rest of modules ...
```

### 6. Test with Real Map Data

Run the application:

```bash
cargo run
```

Expected behavior:
- Loading screen appears
- Console shows: "Loading Town of Endgame from RPGMaker data"
- Console shows: "Loaded map: Town of Endgame (34x39)"
- Map renders with actual tile data from Map002.json
- NPCs spawn at correct positions from event data
- Walk to NPCs and press E
- Dialogue shows actual text from original game

### 7. Copy Required Assets

Ensure all referenced assets are copied:

```bash
# Copy tilesets
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/tilesets/*.png \
   assets/textures/tilesets/

# Copy character sprites
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/characters/*.png \
   assets/textures/characters/

# Copy portraits
cp /home/atobey/src/endgame-of-sre-rpgmaker-mz/img/faces/*.png \
   assets/textures/portraits/
```

## Success Criteria

- [ ] `src/rpgmaker_data.rs` created with JSON parsing structures
- [ ] Map loads from Map002.json successfully
- [ ] Tilemap renders with actual game tiles
- [ ] NPCs spawn at positions from event data
- [ ] NPC dialogue matches original game text
- [ ] Character portraits display correctly
- [ ] Can walk around and interact with all NPCs
- [ ] Team Marathon map (Map004.json) also loadable
- [ ] No compilation errors or warnings

## RPGMaker Tile ID Encoding Reference

RPGMaker MZ uses a complex tile ID system:
- **0-255**: Standard tiles (A1-A5 autotiles)
- **256-8191**: Standard tileset tiles
- **8192+**: Higher layers and flags

For MVP, simplified conversion:
```rust
fn rpgmaker_tile_to_index(tile_id: u32) -> u32 {
    if tile_id >= 2048 {
        // Tileset B-E
        tile_id - 2048
    } else if tile_id >= 1536 {
        // Tileset A5
        tile_id - 1536
    } else {
        0 // Default to empty
    }
}
```

Full tile decoding can be added later for autotiles.

## Known Issues / Future Improvements

- **Simplified tile conversion**: Doesn't handle autotiles or tile flags
- **Single layer**: Only renders layer 0 (ground), ignores decoration layers
- **No collision data**: Collision still hardcoded, not loaded from tileset properties
- **Manual speaker names**: Speaker names not stored in RPGMaker, need manual mapping
- **No event triggers**: Only loads dialogue NPCs, not other event types

## Team Marathon NPCs (Map004.json)

Expected NPCs in Team Marathon:
1. **Nyaanager Evie** (Nature sprite) - Manager
2. **Hidaslo Xela** - SLO specialist
3. **Seventh Daughter of Nine** - Post-mortem writer
4. **Ocean** - Pair programming advocate
5. **Luna** - Pair programming advocate
6. **Doctor McFire** - Incident commander
7. **Rick** - Team member

Verify these spawn correctly after porting Map004.

## Next Steps

After completing this task:
1. Implement Team Marathon map loading (same process as Town of Endgame)
2. Add scene transition triggers (doors between maps)
3. Improve tile rendering to handle multiple layers
4. Load collision data from tileset properties
5. Add remaining maps (Team Disco, Team Inferno, Mahogany Row)

## Dialogue Cleanup

Some RPGMaker dialogues may need editing:
- Remove word-wrap tags: `<WordWrap>` (not needed in Bevy)
- Convert icon codes: `\I[84]` → handle separately
- Split long lines if needed

This can be done programmatically or manually in a dialogue editing pass.

## Notes for Implementation

- RPGMaker stores maps in row-major order: `data[y * width + x]`
- Event IDs start at 1, but array indices start at 0 (events[0] is event ID 1)
- Portrait indices aren't used in our implementation (we load full portrait images)
- Some events may have multiple pages (dialogue changes based on conditions) - we use page 0

## Reference Files

- Map data: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Map*.json`
- Tileset info: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/Tilesets.json`
- Map hierarchy: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/data/MapInfos.json`
- Assets: `/home/atobey/src/endgame-of-sre-rpgmaker-mz/img/`

## Debugging Tips

If NPCs don't spawn:
- Check console for "Unknown character sprite" warnings
- Verify sprite files copied to `assets/textures/characters/`
- Check event has non-empty `character_name`

If dialogue is empty:
- Run `extract_dialogue.py` to see what's in the JSON
- Check code 101 (face) and 401 (text) commands exist
- Some events might be triggers, not dialogue NPCs
