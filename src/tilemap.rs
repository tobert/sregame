use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use crate::game_state::Scene;
use crate::camera::{MainCamera, CameraFollow, CameraBounds};
use crate::npc::{spawn_npc, Npc, NpcDialogue};
use crate::transitions::Door;
use crate::instrumentation::GameTracer;
use crate::assets::GameAssets;
use crate::map_data::{MapData, ExitData, tile_to_world, facing_from_string};
use crate::player::Player;

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ecs_tilemap::TilemapPlugin)
            .add_systems(OnEnter(Scene::TownOfEndgame), spawn_map)
            .add_systems(OnEnter(Scene::TeamMarathon), spawn_map)
            .add_systems(OnEnter(Scene::TeamMarathonRetro), spawn_map)
            .add_systems(OnEnter(Scene::TeamDisco), spawn_map)
            .add_systems(OnEnter(Scene::TeamInferno), spawn_map)
            .add_systems(OnEnter(Scene::MahoganyRow), spawn_map)
            .add_systems(OnEnter(Scene::Intro), spawn_map)
            .add_systems(OnEnter(Scene::End), spawn_map)
            .add_systems(OnExit(Scene::TownOfEndgame), despawn_map)
            .add_systems(OnExit(Scene::TeamMarathon), despawn_map)
            .add_systems(OnExit(Scene::TeamMarathonRetro), despawn_map)
            .add_systems(OnExit(Scene::TeamDisco), despawn_map)
            .add_systems(OnExit(Scene::TeamInferno), despawn_map)
            .add_systems(OnExit(Scene::MahoganyRow), despawn_map)
            .add_systems(OnExit(Scene::Intro), despawn_map)
            .add_systems(OnExit(Scene::End), despawn_map)
            .add_systems(Update, pulse_interact_indicators);
    }
}

#[derive(Component)]
pub struct Map;

/// Pulsing "interact here" tile highlight (see `MapData::indicators`).
#[derive(Component)]
pub struct InteractIndicator;

/// Alpha of the interact glow at time `t` seconds: a slow breath between
/// 0.10 and 0.45 with a ~1.6s period - visible at a glance, calm enough not
/// to upstage the pixel art.
fn indicator_alpha(t: f32) -> f32 {
    0.275 + 0.175 * (t * std::f32::consts::TAU / 1.6).sin()
}

fn pulse_interact_indicators(
    time: Res<Time>,
    mut indicators: Query<&mut Sprite, With<InteractIndicator>>,
) {
    let alpha = indicator_alpha(time.elapsed_secs());
    for mut sprite in &mut indicators {
        sprite.color.set_alpha(alpha);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TileCollision {
    Walkable,
    Blocked,
}

/// Directional passability bits, matching both RPGMaker's flag nibble and
/// the `passability` masks baked by tools/convert_maps.py. A set bit means
/// "can move OUT of this cell in that direction". "Down" is +y in RPGMaker
/// orientation (toward the bottom of the map).
pub const PASS_DOWN: u8 = 0x01;
pub const PASS_LEFT: u8 = 0x02;
pub const PASS_RIGHT: u8 = 0x04;
pub const PASS_UP: u8 = 0x08;
pub const PASS_ALL: u8 = 0x0F;

#[derive(Resource)]
pub struct CollisionMap {
    pub width: u32,
    pub height: u32,
    /// Per-cell 4-bit masks, RPGMaker orientation (row 0 = top).
    passability: Vec<u8>,
    /// Counter cells (RPGMaker tile flag 0x80): the action button reaches
    /// one tile across these. Filled from MapData::counters by spawn_map.
    pub counters: std::collections::HashSet<(i32, i32)>,
}

impl CollisionMap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            passability: vec![PASS_ALL; (width * height) as usize],
            counters: Default::default(),
        }
    }

    pub fn from_passability(width: u32, height: u32, passability: Vec<u8>) -> Self {
        assert_eq!(
            passability.len(),
            (width * height) as usize,
            "passability data doesn't match map dimensions"
        );
        Self { width, height, passability, counters: Default::default() }
    }

    /// RPGMaker's Game_Map.isCounter.
    pub fn is_counter(&self, x: i32, y: i32) -> bool {
        self.counters.contains(&(x, y))
    }

    pub fn set_tile(&mut self, x: u32, y: u32, collision: TileCollision) {
        if x < self.width && y < self.height {
            let index = (y * self.width + x) as usize;
            self.passability[index] = match collision {
                TileCollision::Walkable => PASS_ALL,
                TileCollision::Blocked => 0,
            };
        }
    }

    fn mask(&self, x: i32, y: i32) -> Option<u8> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        self.passability
            .get((y as u32 * self.width + x as u32) as usize)
            .copied()
    }

    /// "Could the player stand here at all" - true if the cell is enterable
    /// from at least one direction. Prefer `can_step` for movement.
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.mask(x, y).is_some_and(|m| m != 0)
    }

    /// Test-only direct mask access; production masks come from the baked
    /// map JSON via `from_passability`.
    #[cfg(test)]
    pub fn passability_for_tests(&mut self, x: u32, y: u32, mask: u8) {
        let index = (y * self.width + x) as usize;
        self.passability[index] = mask;
    }

    /// RPGMaker's Game_CharacterBase.canPass for one axis-aligned tile
    /// step: the source cell's exit edge AND the destination cell's entry
    /// edge must both be open. This is what makes one-way tiles work -
    /// shop counters and wall bands are enterable from some sides only,
    /// which no single per-cell boolean can express. Non-adjacent or
    /// diagonal steps fail closed (movement is applied per axis).
    pub fn can_step(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        if from == to {
            return true;
        }
        let (exit_bit, entry_bit) = match (to.0 - from.0, to.1 - from.1) {
            (0, 1) => (PASS_DOWN, PASS_UP),
            (0, -1) => (PASS_UP, PASS_DOWN),
            (-1, 0) => (PASS_LEFT, PASS_RIGHT),
            (1, 0) => (PASS_RIGHT, PASS_LEFT),
            _ => return false,
        };
        let (Some(from_mask), Some(to_mask)) = (self.mask(from.0, from.1), self.mask(to.0, to.1)) else {
            return false;
        };
        from_mask & exit_bit != 0 && to_mask & entry_bit != 0
    }
}

/// Exit (portal) triggers for the currently loaded map. Same resource
/// lifecycle as `CollisionMap`: inserted by `spawn_map`, removed by
/// `despawn_map`.
#[derive(Resource)]
pub struct MapExits(pub Vec<ExitData>);

/// Set by the transition system just before switching scenes; consumed by
/// `spawn_map` on the following `OnEnter` to place the player at the correct
/// tile in the newly-loaded map. Absent on the very first scene load, since
/// there's no incoming transition then.
#[derive(Resource)]
pub struct PendingArrival {
    pub spawn_x: u32,
    pub spawn_y: u32,
}

/// Per-scene map file + tileset lookup. Tileset keys are a contract with
/// `assets::GameAssets::tilesets` (populated by scanning
/// `assets/textures/tilesets/*.png`): "town_tileset" for the outdoor Town of
/// Endgame map, "inside_tileset" for all interior maps.
pub struct SceneConfig {
    pub map_file: &'static str,
    pub tileset_key: &'static str,
}

pub fn scene_config(scene: Scene) -> SceneConfig {
    match scene {
        Scene::TownOfEndgame => SceneConfig { map_file: "town_of_endgame", tileset_key: "town_tileset" },
        Scene::TeamMarathon => SceneConfig { map_file: "team_marathon", tileset_key: "inside_tileset" },
        Scene::TeamMarathonRetro => SceneConfig { map_file: "team_marathon_retro", tileset_key: "inside_tileset" },
        Scene::TeamDisco => SceneConfig { map_file: "team_disco", tileset_key: "inside_tileset" },
        Scene::TeamInferno => SceneConfig { map_file: "team_inferno", tileset_key: "inside_tileset" },
        Scene::MahoganyRow => SceneConfig { map_file: "mahogany_row", tileset_key: "inside_tileset" },
        Scene::Intro => SceneConfig { map_file: "intro", tileset_key: "inside_tileset" },
        Scene::End => SceneConfig { map_file: "end", tileset_key: "inside_tileset" },
    }
}

fn spawn_map(
    mut commands: Commands,
    scene: Res<State<Scene>>,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut camera_query: Query<&mut CameraFollow, With<MainCamera>>,
    mut player_query: Query<&mut Transform, With<Player>>,
    pending_arrival: Option<Res<PendingArrival>>,
    tracer: Option<Res<GameTracer>>,
) {
    let config = scene_config(*scene.get());

    info!("Loading {:?} from map data ({})", scene.get(), config.map_file);

    let map = match MapData::load(config.map_file) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to load map '{}': {:?}", config.map_file, e);
            // Don't leave a stale PendingArrival around for some later,
            // unrelated scene load to accidentally consume - a portal that
            // led nowhere shouldn't silently misplace the player next time
            // a map *does* load successfully.
            if pending_arrival.is_some() {
                warn!("Discarding PendingArrival - target scene's map failed to load");
                commands.remove_resource::<PendingArrival>();
            }
            return;
        }
    };

    info!("Loaded map: {} ({}x{})", map.name, map.width, map.height);

    // A missing tileset is a visual gap, not a logical one: the map's
    // collision, exits and NPCs must still come up so the transition system
    // works even for scenes whose art hasn't been authored yet (several
    // interior scenes don't have clean map JSON *or* art yet - see
    // scene_config). Fall back to an empty texture handle and keep going.
    let texture_handle = match game_assets.tilesets.get(config.tileset_key).cloned() {
        Some(handle) => handle,
        None => {
            warn!(
                "Missing tileset '{}' for scene {:?} - rendering without tile art",
                config.tileset_key, scene.get()
            );
            Handle::default()
        }
    };

    const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 48.0, y: 48.0 };
    const GRID_SIZE: TilemapGridSize = TilemapGridSize { x: 48.0, y: 48.0 };
    // Ground and upper layers share one atlas (see tools/convert_maps.py),
    // so both TilemapBundles below reference the same texture handle.
    // Upper renders above the player/NPCs (z=1.0, see player.rs/npc.rs).
    const GROUND_Z: f32 = 0.0;
    const UPPER_Z: f32 = 2.0;

    let map_size = TilemapSize { x: map.width, y: map.height };

    let ground_entity = commands.spawn_empty().id();
    let mut ground_storage = TileStorage::empty(map_size);

    let upper_entity = commands.spawn_empty().id();
    let mut upper_storage = TileStorage::empty(map_size);

    for y in 0..map.height {
        for x in 0..map.width {
            // Map JSON rows are RPGMaker-ordered (row 0 = top), while
            // bevy_ecs_tilemap's TilePos y=0 is the BOTTOM row, so the row
            // must be flipped here or the whole map renders vertically
            // mirrored. Same convention boundary as map_data::tile_to_world.
            let tile_pos = TilePos { x, y: map.height - 1 - y };
            let index = (y * map.width + x) as usize;

            let ground_index = map.tiles.get(index).copied().unwrap_or(0);
            let ground_tile = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(ground_entity),
                        texture_index: TileTextureIndex(ground_index),
                        ..default()
                    },
                    Map,
                ))
                .id();
            ground_storage.set(&tile_pos, ground_tile);

            let upper_index = map.upper_tiles.get(index).copied().unwrap_or(0);
            let upper_tile = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(upper_entity),
                        texture_index: TileTextureIndex(upper_index),
                        ..default()
                    },
                    Map,
                ))
                .id();
            upper_storage.set(&tile_pos, upper_tile);
        }
    }

    // TilemapAnchor::Center, NOT a hand-rolled -(W*48)/2 transform: the
    // tilemap's native origin is the CENTER of the bottom-left tile (see
    // bevy_ecs_tilemap::anchor), so the old manual offset rendered the whole
    // map half a tile (24px) down-left of where tile_to_world - and
    // therefore collision, NPCs, exits, and the player - believed tiles
    // were. Felt like "collision is shifted" in playtesting. With Center,
    // rendered tile centers coincide exactly with tile_to_world output.
    commands.entity(ground_entity).insert((
        TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: ground_storage,
            texture: TilemapTexture::Single(texture_handle.clone()),
            tile_size: TILE_SIZE,
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, GROUND_Z),
            ..default()
        },
        Map,
    ));

    commands.entity(upper_entity).insert((
        TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: upper_storage,
            texture: TilemapTexture::Single(texture_handle),
            tile_size: TILE_SIZE,
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, UPPER_Z),
            ..default()
        },
        Map,
    ));

    // CollisionMap stays in RPGMaker orientation (y=0 = top row, same as the
    // JSON), because every lookup goes through world_to_tile, which returns
    // RPGMaker-orientation coordinates. Directional masks when the JSON has
    // them; coarse blocked/walkable fallback for older JSON.
    let cell_count = (map.width * map.height) as usize;
    let mut collision_map = if map.passability.len() == cell_count {
        CollisionMap::from_passability(map.width, map.height, map.passability.clone())
    } else {
        if !map.passability.is_empty() {
            warn!(
                "Map '{}' passability has {} cells, expected {} - falling back to collision",
                map.name, map.passability.len(), cell_count
            );
        }
        let mut fallback = CollisionMap::new(map.width, map.height);
        for y in 0..map.height {
            for x in 0..map.width {
                let index = (y * map.width + x) as usize;
                if map.collision.get(index).copied().unwrap_or(true) {
                    fallback.set_tile(x, y, TileCollision::Blocked);
                }
            }
        }
        fallback
    };
    // Blocking props (The Boss's Truck): RPGMaker events with priority
    // "same as characters" and through=false are impassable, and the
    // tile-flag bake can't know about events.
    for prop in map.props.iter().filter(|p| p.blocks) {
        collision_map.set_tile(prop.x, prop.y, TileCollision::Blocked);
    }
    collision_map.counters = map
        .counters
        .iter()
        .map(|&(x, y)| (x as i32, y as i32))
        .collect();
    // NPCs deliberately do NOT bake into the tile map (they used to):
    // a full-tile block read as a boundary wider than the NPC's body yet
    // short enough for sprites to overlap vertically. They collide as
    // body-shaped AABBs against the player instead - see npc_blocks_move
    // in player.rs. (Verified against the original: every NPC event is
    // priority 1 / through=false; only doggo is through, and doggo is a
    // prop.)
    commands.insert_resource(collision_map);
    commands.insert_resource(MapExits(map.exits.clone()));

    if let Ok(mut camera_follow) = camera_query.single_mut() {
        let map_width_pixels = map.width as f32 * TILE_SIZE.x;
        let map_height_pixels = map.height as f32 * TILE_SIZE.y;

        camera_follow.bounds = Some(CameraBounds::from_map_size(
            map_width_pixels,
            map_height_pixels,
        ));
    }

    // Spawn NPCs from map data
    info!("Spawning {} NPCs from map data", map.npcs.len());
    for npc_data in &map.npcs {
        let world_pos = tile_to_world(npc_data.x, npc_data.y, map.width, map.height);

        // Map sprite name to asset handle, looked up by filename stem from
        // the data-driven GameAssets::npc_sprites map.
        let Some(sprite_handle) = game_assets.npc_sprites.get(&npc_data.sprite).cloned() else {
            warn!("Unknown NPC sprite: {} - skipping {}", npc_data.sprite, npc_data.name);
            continue;
        };

        let portrait_path = if !npc_data.dialogue.portrait.is_empty() {
            format!("textures/portraits/{}.png", npc_data.dialogue.portrait)
        } else {
            String::new()
        };

        let npc_entity = spawn_npc(
            &mut commands,
            &game_assets,
            &mut texture_atlas_layouts,
            Vec3::new(world_pos.x, world_pos.y, 1.0),
            sprite_handle,
            Npc {
                name: npc_data.name.clone(),
                sprite_facing: facing_from_string(&npc_data.facing),
                sprite_slot: npc_data.sprite_index,
            },
            npc_data.step_anime,
            NpcDialogue {
                speaker: npc_data.dialogue.speaker.clone(),
                portrait_path,
                portrait_face_index: npc_data.dialogue.face_index,
                lines: npc_data.dialogue.lines.clone(),
            },
            tracer.as_deref(),
        );
        // Map marker so despawn_map removes NPCs on scene exit. Without it
        // NPCs leaked across transitions - live but offscreen in the next
        // map, complete with their Interactable zones (ghost dialogues).
        // Found via a mid-transfer BRP screenshot: a town NPC rendered in
        // the void outside the destination room.
        commands.entity(npc_entity).insert(Map);
        // Solid body unless the original event is Through (doggo): the
        // player's NPC collision (player.rs::npc_blocks_move) only sees
        // NpcBody carriers.
        if !npc_data.through {
            commands.entity(npc_entity).insert(crate::npc::NpcBody);
        }
        if npc_data.wander {
            commands.entity(npc_entity).insert(crate::npc::Wanderer::default());
        }

        info!("Spawned NPC: {} at tile ({}, {})", npc_data.name, npc_data.x, npc_data.y);
    }

    // Door sprites on exit trigger tiles (visual only - exit logic is in
    // MapExits; the open animation is driven by transitions.rs).
    for door in &map.doors {
        let Some(handle) = game_assets.npc_sprites.get(&door.sprite).cloned() else {
            warn!("Unknown door sprite: {} - skipping door at ({}, {})",
                door.sprite, door.x, door.y);
            continue;
        };

        let layout = texture_atlas_layouts.add(
            crate::character_sheet::sheet_layout_with_frame(
                UVec2::new(door.frame_width, door.frame_height),
            ),
        );
        let index = crate::character_sheet::atlas_index(
            door.sprite_index,
            facing_from_string(&door.facing) as u32,
            door.pattern,
        ) as usize;

        let world_pos = tile_to_world(door.x, door.y, map.width, map.height);
        // Tall frames anchor to the tile's bottom edge like RPGMaker: a
        // 48x96 door covers its trigger tile plus the tile above it.
        let y_offset = (door.frame_height as f32 - TILE_SIZE.y) / 2.0;

        commands.spawn((
            Sprite::from_atlas_image(handle, TextureAtlas { layout, index }),
            // Below the player/NPCs (z=1.0): the player walking onto the
            // door tile draws in front of the opening door, as in RPGMaker.
            Transform::from_xyz(world_pos.x, world_pos.y + y_offset, 0.9),
            Map,
            Door {
                tile_x: door.x,
                tile_y: door.y,
                sprite_slot: door.sprite_index,
                pattern: door.pattern,
            },
        ));

        info!("Spawned door at tile ({}, {})", door.x, door.y);
    }

    // Ambient props (doggo, The Boss's Truck): same sheet slicing as doors,
    // no interaction. step_anime props bob in place via the shared
    // CharacterFrames + StepAnimation systems in npc.rs.
    for prop in &map.props {
        let Some(handle) = game_assets.npc_sprites.get(&prop.sprite).cloned() else {
            warn!("Unknown prop sprite: {} - skipping {}", prop.sprite, prop.name);
            continue;
        };

        let layout = texture_atlas_layouts.add(
            crate::character_sheet::sheet_layout_with_frame(
                UVec2::new(prop.frame_width, prop.frame_height),
            ),
        );
        let facing_row = facing_from_string(&prop.facing) as u32;
        let index = crate::character_sheet::atlas_index(
            prop.sprite_index,
            facing_row,
            prop.pattern,
        ) as usize;

        let world_pos = tile_to_world(prop.x, prop.y, map.width, map.height);
        let y_offset = (prop.frame_height as f32 - TILE_SIZE.y) / 2.0;

        let mut prop_commands = commands.spawn((
            Sprite::from_atlas_image(handle, TextureAtlas { layout, index }),
            Transform::from_xyz(world_pos.x, world_pos.y + y_offset, 0.95),
            crate::npc::CharacterFrames { slot: prop.sprite_index, facing_row },
            // Feet at the tile the prop stands on, not its lifted center -
            // a 48x96 truck must y-sort by its ground line (see depth.rs).
            crate::depth::YSorted { foot_offset: -(prop.frame_height as f32) / 2.0 },
            Map,
        ));
        if prop.step_anime {
            prop_commands.insert(crate::npc::StepAnimation::default());
        }

        info!("Spawned prop: {} at tile ({}, {})", prop.name, prop.x, prop.y);
    }

    // Pulsing "interact here" highlights (see MapData::indicators): soft
    // warm overlays whose alpha breathes. Decoupled from exit triggers so
    // the glow can sit on the eye-catching graphic (the retro table's
    // parchment) while the trigger stays on the walkable tiles.
    for &(x, y) in &map.indicators {
        let world_pos = tile_to_world(x, y, map.width, map.height);
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 0.93, 0.5, 0.35),
                custom_size: Some(Vec2::new(TILE_SIZE.x - 6.0, TILE_SIZE.y - 6.0)),
                ..default()
            },
            // Above doors (0.9), below props (0.95) and the character band
            // (1.0±) - the glow hugs the ground under sprites and bodies.
            Transform::from_xyz(world_pos.x, world_pos.y, 0.93),
            InteractIndicator,
            Map,
        ));
        info!("Spawned interact indicator at tile ({x}, {y})");
    }

    // If we arrived via a portal (see transitions.rs), place the player at
    // the target spawn tile. If absent, this is either the very first scene
    // load or a scene the player didn't reach via a portal - leave the
    // player wherever it already is.
    if let Some(arrival) = pending_arrival {
        if let Ok(mut player_transform) = player_query.single_mut() {
            let spawn_pos = tile_to_world(arrival.spawn_x, arrival.spawn_y, map.width, map.height);
            player_transform.translation.x = spawn_pos.x;
            player_transform.translation.y = spawn_pos.y;
            info!("Placed player at incoming spawn tile ({}, {})", arrival.spawn_x, arrival.spawn_y);
        }
        commands.remove_resource::<PendingArrival>();
    }
}

fn despawn_map(
    mut commands: Commands,
    map_query: Query<Entity, With<Map>>,
) {
    for entity in &map_query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<CollisionMap>();
    commands.remove_resource::<MapExits>();
    // A door departure that caused this teardown holds player input frozen
    // until the scene actually swaps; release it here.
    commands.remove_resource::<crate::transitions::DepartingDoor>();
    info!("Map despawned");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The interact glow must stay visible-but-subtle across its whole
    /// cycle: never invisible (min > 0), never poster-bright (max < 0.5),
    /// and actually pulsing (meaningful swing between extremes).
    #[test]
    fn indicator_pulse_stays_in_the_subtle_band() {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for i in 0..320 {
            let a = indicator_alpha(i as f32 * 0.0125); // 4s @ 80Hz
            min = min.min(a);
            max = max.max(a);
        }
        assert!(min > 0.05 && min < 0.15, "min alpha {min} out of band");
        assert!(max > 0.40 && max < 0.50, "max alpha {max} out of band");
        assert!(max - min > 0.3, "pulse swing {} too subtle to notice", max - min);
    }

    #[test]
    fn can_step_respects_one_way_edges() {
        // A "shop counter" cell: enterable/exitable down, left, right - but
        // not up. Approaching from the south moving north is what the item
        // shop's counter blocks in the original.
        let mut map = CollisionMap::new(3, 3);
        // center cell (1,1) = counter with mask down|left|right
        map.passability_for_tests(1, 1, PASS_DOWN | PASS_LEFT | PASS_RIGHT);

        // From below (1,2) moving up: exit up of floor is open, but entry
        // means the counter's DOWN edge... which is open - RPGMaker lets
        // you step ONTO such a cell from below; what it forbids is leaving
        // upward. Verify the exact engine semantics:
        assert!(map.can_step((1, 2), (1, 1)), "stepping onto the counter from below is legal");
        assert!(!map.can_step((1, 1), (1, 0)), "leaving the counter upward is not");

        // A one-way 'dr' storefront edge (town (23,2)): enterable from
        // below/right, NOT from the left.
        let mut town = CollisionMap::new(3, 3);
        town.passability_for_tests(1, 1, PASS_DOWN | PASS_RIGHT);
        assert!(!town.can_step((0, 1), (1, 1)), "entering a 'dr' cell moving right must fail (its left edge is closed)");
        assert!(town.can_step((1, 2), (1, 1)), "entering the same cell from below is fine");
    }

    #[test]
    fn can_step_fails_closed_at_map_edges_and_diagonals() {
        let map = CollisionMap::new(2, 2);
        assert!(!map.can_step((0, 0), (-1, 0)), "off-map is never passable");
        assert!(!map.can_step((0, 0), (1, 1)), "diagonal steps fail closed");
        assert!(map.can_step((0, 0), (0, 0)), "staying put is always fine");
    }

    #[test]
    fn set_tile_blocked_closes_every_edge() {
        let mut map = CollisionMap::new(2, 1);
        map.set_tile(1, 0, TileCollision::Blocked);
        assert!(!map.can_step((0, 0), (1, 0)));
        assert!(!map.is_walkable(1, 0));
        map.set_tile(1, 0, TileCollision::Walkable);
        assert!(map.can_step((0, 0), (1, 0)));
    }
}
