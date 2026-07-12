use bevy::prelude::*;
use serde::Deserialize;
use anyhow::{Context, Result};
use std::fs;

#[derive(Debug, Deserialize)]
pub struct MapData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Ground-layer atlas indices, one per cell (row-major, RPGMaker
    /// orientation: row 0 is the TOP row of the map, matching the source
    /// Map*.json data planes). Index 0 is a reserved fully-transparent tile.
    /// The top-down -> bottom-up (+y up) conversion happens exactly once, at
    /// the world boundary: `tile_to_world`/`world_to_tile` here and the
    /// `TilePos` mapping in tilemap.rs::spawn_map.
    pub tiles: Vec<u32>,
    /// Upper-layer (drawn above the player/NPCs) atlas indices into the
    /// *same* atlas as `tiles`, same shape as `tiles`. 0 means "no
    /// upper-layer decoration on this cell". Produced by
    /// tools/convert_maps.py from RPGMaker's per-tile 0x10 "higher" flag.
    /// Defaults to empty for map JSON predating this field; an absent index
    /// renders as tile 0 (blank), same as an explicit empty array.
    #[serde(default)]
    pub upper_tiles: Vec<u32>,
    /// Per-cell fully-blocked flag (row-major, same shape as `tiles`),
    /// baked from RPGMaker tileset passability flags by
    /// tools/convert_maps.py. See CollisionMap in tilemap.rs. Defaults to
    /// empty for map JSON predating this field; an absent index then reads
    /// as blocked (fail closed) via `.unwrap_or(true)` in tilemap.rs, not
    /// walkable.
    #[serde(default)]
    pub collision: Vec<bool>,
    pub npcs: Vec<NpcData>,
    #[serde(default)]
    pub exits: Vec<ExitData>,
    /// Visible door sprites sitting on exit trigger tiles (the town's
    /// `!doors` events). Purely visual - the exit logic itself lives in
    /// `exits`. Defaults to empty for map JSON predating this field.
    #[serde(default)]
    pub doors: Vec<DoorData>,
    /// Ambient visual props: image-bearing events with no dialogue and no
    /// transfer (doggo, The Boss's Truck). Defaults to empty for map JSON
    /// predating this field.
    #[serde(default)]
    pub props: Vec<PropData>,
}

/// One ambient prop sprite. Same sheet-slicing rules as `DoorData`;
/// `blocks` carries RPGMaker's event collision (priority "same as
/// characters" + through=false makes the event's tile impassable, which our
/// tile-flag-baked collision can't know about).
#[derive(Debug, Clone, Deserialize)]
pub struct PropData {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub sprite: String,
    pub sprite_index: u32,
    pub facing: String,
    pub pattern: u32,
    #[serde(default)]
    pub step_anime: bool,
    #[serde(default)]
    pub blocks: bool,
    pub frame_width: u32,
    pub frame_height: u32,
}

/// One door sprite. `frame_width`/`frame_height` are baked by
/// tools/convert_maps.py from the sheet's dimensions (RPGMaker frames are
/// sheet_width/12 x sheet_height/8; doors.png is 576x768, so door frames
/// are 48x96 - one tile wide, two tiles tall). `facing` is the resting
/// animation row ("down" = closed); `pattern` the resting column.
#[derive(Debug, Clone, Deserialize)]
pub struct DoorData {
    pub x: u32,
    pub y: u32,
    pub sprite: String,
    pub sprite_index: u32,
    pub facing: String,
    pub pattern: u32,
    pub frame_width: u32,
    pub frame_height: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExitData {
    pub trigger_x: u32,
    pub trigger_y: u32,
    /// Matches a `Scene` variant name (e.g. "TeamMarathon"), see `scene_from_str`.
    pub target_scene: String,
    pub target_spawn_x: u32,
    pub target_spawn_y: u32,
}

#[derive(Debug, Deserialize)]
pub struct NpcData {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub sprite: String,
    /// Which character slot (0-7) of the `sprite` sheet this NPC uses -
    /// RPGMaker MZ's `image.characterIndex` (sheets hold a 4x2 grid of
    /// characters, see character_sheet.rs). Defaults to 0 (top-left slot)
    /// for map JSON predating this field.
    #[serde(default)]
    pub sprite_index: u32,
    /// RPGMaker's "Stepping Animation": play the walk cycle in place while
    /// standing still. Nearly every NPC in the original has this enabled.
    /// Defaults to false (a statue) for map JSON predating this field.
    #[serde(default)]
    pub step_anime: bool,
    pub facing: String,
    pub dialogue: DialogueData,
}

#[derive(Debug, Deserialize)]
pub struct DialogueData {
    pub speaker: String,
    pub portrait: String,
    /// Which cell of `portrait`'s face sheet to display (RPGMaker MZ code-101
    /// "Show Face" `faceIndex`, 0-7 in the standard 4-column x 2-row 144x144px
    /// grid layout - see tools/convert_maps.py's
    /// `extract_dialogue_from_commands`). Defaults to 0 (top-left cell) for
    /// map JSON predating this field.
    #[serde(default)]
    pub face_index: u32,
    pub lines: Vec<String>,
}

impl MapData {
    pub fn load(map_name: &str) -> Result<Self> {
        let path = format!("assets/data/maps/{}.json", map_name);
        let json = fs::read_to_string(&path)
            .context(format!("Failed to read map file: {}", path))?;

        let map: MapData = serde_json::from_str(&json)
            .context("Failed to parse map JSON")?;

        Ok(map)
    }
}

/// Converts a tile coordinate in RPGMaker orientation (y = 0 is the TOP row,
/// y grows downward - the convention all map JSON, NPC, and exit data is
/// stored in) to a Bevy world-space position (+y is up, map centered on the
/// origin). This flip is deliberately done here and nowhere else; feeding
/// unflipped tile y through (as an earlier version did) renders every map
/// vertically mirrored.
pub fn tile_to_world(tile_x: u32, tile_y: u32, map_width: u32, map_height: u32) -> Vec2 {
    const TILE_SIZE: f32 = 48.0;

    let world_x = (tile_x as f32 - map_width as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;
    let flipped_y = map_height as f32 - 1.0 - tile_y as f32;
    let world_y = (flipped_y - map_height as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;

    Vec2::new(world_x, world_y)
}

/// Inverse of `tile_to_world`: maps a world-space position back to the
/// RPGMaker-orientation tile coordinate (y = 0 at the top) that contains it,
/// for a map of the given dimensions.
pub fn world_to_tile(world_pos: Vec2, map_width: u32, map_height: u32) -> (i32, i32) {
    const TILE_SIZE: f32 = 48.0;

    let tile_x = (world_pos.x / TILE_SIZE + map_width as f32 / 2.0).floor() as i32;
    let tile_y_bottom_up = (world_pos.y / TILE_SIZE + map_height as f32 / 2.0).floor() as i32;
    let tile_y = map_height as i32 - 1 - tile_y_bottom_up;

    (tile_x, tile_y)
}

pub fn facing_from_string(facing: &str) -> crate::npc::NpcFacing {
    match facing {
        "down" => crate::npc::NpcFacing::Down,
        "left" => crate::npc::NpcFacing::Left,
        "right" => crate::npc::NpcFacing::Right,
        "up" => crate::npc::NpcFacing::Up,
        _ => crate::npc::NpcFacing::Down,
    }
}

/// Maps a clean-JSON `target_scene` string (e.g. "TeamMarathon") to a `Scene`
/// variant. Mirrors the `facing_from_string` idiom above. Returns `None` for
/// unrecognized names so callers can log and skip rather than silently
/// defaulting to the wrong scene.
pub fn scene_from_str(name: &str) -> Option<crate::game_state::Scene> {
    use crate::game_state::Scene;

    match name {
        "TownOfEndgame" => Some(Scene::TownOfEndgame),
        "TeamMarathon" => Some(Scene::TeamMarathon),
        "TeamMarathonRetro" => Some(Scene::TeamMarathonRetro),
        "TeamDisco" => Some(Scene::TeamDisco),
        "TeamInferno" => Some(Scene::TeamInferno),
        "MahoganyRow" => Some(Scene::MahoganyRow),
        "Intro" => Some(Scene::Intro),
        "End" => Some(Scene::End),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::Scene;

    #[test]
    fn tile_to_world_and_back_round_trips() {
        // Covers corners, map center, and the real trigger/spawn tiles used
        // by the Town of Endgame <-> Team Marathon portal pair, so a broken
        // world_to_tile (the inverse used by the transition system) fails
        // loudly instead of silently landing the player on the wrong tile.
        let cases: &[(u32, u32, u32, u32)] = &[
            (0, 0, 34, 39),
            (33, 38, 34, 39),
            (17, 19, 34, 39),
            (30, 30, 34, 39),
            (12, 15, 24, 21),
            (12, 16, 24, 21),
            (8, 30, 34, 39),
        ];

        for &(tile_x, tile_y, width, height) in cases {
            let world = tile_to_world(tile_x, tile_y, width, height);
            let (round_tripped_x, round_tripped_y) = world_to_tile(world, width, height);
            assert_eq!(
                (tile_x as i32, tile_y as i32),
                (round_tripped_x, round_tripped_y),
                "tile ({tile_x}, {tile_y}) on a {width}x{height} map didn't round-trip \
                 through world space (got world {world:?})"
            );
        }
    }

    #[test]
    fn tile_y_zero_is_the_top_of_the_world_map() {
        // Map data is stored in RPGMaker orientation: row 0 is the TOP of
        // the map. Bevy world space has +y up, so row 0 must land in the
        // top (positive-y) half of the world and increasing tile y must
        // move DOWN in world y. This is the regression test for the
        // vertical-mirroring bug where every map rendered upside down
        // (roofs below doors) because tile y was fed through unflipped.
        let (width, height) = (34, 39);

        let top_left = tile_to_world(0, 0, width, height);
        assert!(top_left.x < 0.0, "tile x=0 should be on the left (got {top_left:?})");
        assert!(top_left.y > 0.0, "tile y=0 should be at the TOP (got {top_left:?})");

        let bottom_left = tile_to_world(0, height - 1, width, height);
        assert!(bottom_left.y < 0.0, "last row should be at the BOTTOM (got {bottom_left:?})");

        let one_down = tile_to_world(0, 1, width, height);
        assert!(
            one_down.y < top_left.y,
            "increasing tile y must decrease world y ({} !< {})",
            one_down.y, top_left.y
        );
    }

    #[test]
    fn world_to_tile_is_stable_mid_tile() {
        // A world position partway across a tile (not just its center) must
        // still resolve to that same tile, not an adjacent one - this is
        // what actually happens as the player walks continuously.
        let width = 34;
        let height = 39;
        let tile_center = tile_to_world(10, 10, width, height);

        for offset in [-20.0_f32, -1.0, 0.0, 1.0, 20.0] {
            let nudged = Vec2::new(tile_center.x + offset, tile_center.y + offset);
            assert_eq!(world_to_tile(nudged, width, height), (10, 10));
        }
    }

    #[test]
    fn scene_from_str_maps_all_known_variants() {
        let cases = [
            ("TownOfEndgame", Scene::TownOfEndgame),
            ("TeamMarathon", Scene::TeamMarathon),
            ("TeamMarathonRetro", Scene::TeamMarathonRetro),
            ("TeamDisco", Scene::TeamDisco),
            ("TeamInferno", Scene::TeamInferno),
            ("MahoganyRow", Scene::MahoganyRow),
            ("Intro", Scene::Intro),
            ("End", Scene::End),
        ];

        for (name, expected) in cases {
            assert_eq!(scene_from_str(name), Some(expected), "failed for '{name}'");
        }
    }

    #[test]
    fn scene_from_str_rejects_unknown_names() {
        assert_eq!(scene_from_str("NotARealScene"), None);
        assert_eq!(scene_from_str(""), None);
        assert_eq!(scene_from_str("townofendgame"), None); // case-sensitive
    }

    #[test]
    fn map_data_deserializes_exits() {
        let json = r#"{
            "name": "Test Map",
            "width": 10,
            "height": 10,
            "tiles": [],
            "npcs": [],
            "exits": [
                { "trigger_x": 1, "trigger_y": 2, "target_scene": "TeamMarathon",
                  "target_spawn_x": 3, "target_spawn_y": 4 }
            ]
        }"#;

        let map: MapData = serde_json::from_str(json).expect("valid map JSON should parse");
        assert_eq!(map.exits.len(), 1);
        assert_eq!(map.exits[0].trigger_x, 1);
        assert_eq!(map.exits[0].target_scene, "TeamMarathon");
    }

    #[test]
    fn every_shipped_map_npc_sprite_and_portrait_file_exists() {
        // GameAssets loads sprites/portraits by scanning
        // assets/textures/{characters,portraits}/*.png at startup (see
        // assets.rs) - there's no compile-time check that a map's NPC data
        // references a file that's actually on disk. A missing sprite is a
        // silent "warn + skip" in tilemap.rs (the NPC just never appears),
        // and a missing portrait falls back to no portrait in the dialogue
        // box - both easy to miss when adding a new map's content by hand.
        // This test catches that at `cargo test` time instead of by
        // noticing an NPC silently didn't spawn during a playtest.
        let maps_dir = std::path::Path::new("assets/data/maps");
        let characters_dir = std::path::Path::new("assets/textures/characters");
        let portraits_dir = std::path::Path::new("assets/textures/portraits");

        let mut missing = Vec::new();

        for entry in fs::read_dir(maps_dir).expect("assets/data/maps should exist") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let map_name = path.file_stem().unwrap().to_string_lossy().to_string();
            let map = MapData::load(&map_name).expect("shipped map JSON should parse");

            for npc in &map.npcs {
                if !characters_dir.join(format!("{}.png", npc.sprite)).exists() {
                    missing.push(format!(
                        "{map_name}: NPC '{}' sprite '{}' -> assets/textures/characters/{}.png",
                        npc.name, npc.sprite, npc.sprite
                    ));
                }
                if npc.sprite_index > 7 {
                    missing.push(format!(
                        "{map_name}: NPC '{}' sprite_index {} exceeds the 0-7 \
                         character slots a sheet holds (character_sheet.rs \
                         would panic at spawn)",
                        npc.name, npc.sprite_index
                    ));
                }
                let portrait = &npc.dialogue.portrait;
                if !portrait.is_empty() && !portraits_dir.join(format!("{portrait}.png")).exists() {
                    missing.push(format!(
                        "{map_name}: NPC '{}' portrait '{}' -> assets/textures/portraits/{}.png",
                        npc.name, portrait, portrait
                    ));
                }
            }

            for door in &map.doors {
                if !characters_dir.join(format!("{}.png", door.sprite)).exists() {
                    missing.push(format!(
                        "{map_name}: door at ({}, {}) sprite '{}' -> assets/textures/characters/{}.png",
                        door.x, door.y, door.sprite, door.sprite
                    ));
                }
            }

            for prop in &map.props {
                if !characters_dir.join(format!("{}.png", prop.sprite)).exists() {
                    missing.push(format!(
                        "{map_name}: prop '{}' sprite '{}' -> assets/textures/characters/{}.png",
                        prop.name, prop.sprite, prop.sprite
                    ));
                }
                if prop.sprite_index > 7 {
                    missing.push(format!(
                        "{map_name}: prop '{}' sprite_index {} exceeds the 0-7 slots",
                        prop.name, prop.sprite_index
                    ));
                }
            }
        }

        assert!(missing.is_empty(), "missing NPC art assets:\n{}", missing.join("\n"));
    }

    #[test]
    fn dialogue_data_face_index_round_trips() {
        // A code-101 box referencing e.g. `['casey', 4, 0, 0, 'Boba Jacobian']`
        // (see tools/convert_maps.py::extract_dialogue_from_commands) must
        // carry faceIndex 4 through to DialogueData unchanged, so the
        // dialogue UI crops the correct cell of the face sheet.
        let json = r#"{
            "name": "Test Map",
            "width": 10,
            "height": 10,
            "tiles": [],
            "npcs": [
                {
                    "name": "Boba Jacobian",
                    "x": 1,
                    "y": 2,
                    "sprite": "Actor1",
                    "facing": "down",
                    "dialogue": {
                        "speaker": "Boba Jacobian",
                        "portrait": "casey",
                        "face_index": 4,
                        "lines": ["Hello."]
                    }
                }
            ]
        }"#;

        let map: MapData = serde_json::from_str(json).expect("valid map JSON should parse");
        assert_eq!(map.npcs[0].dialogue.face_index, 4);
    }

    #[test]
    fn dialogue_data_face_index_defaults_to_zero_when_absent() {
        // Older/hand-written map JSON without a "face_index" key must still
        // parse (backward compatible), defaulting to the sheet's top-left
        // cell rather than failing to deserialize.
        let json = r#"{
            "name": "Test Map",
            "width": 10,
            "height": 10,
            "tiles": [],
            "npcs": [
                {
                    "name": "Nature Spirit",
                    "x": 1,
                    "y": 2,
                    "sprite": "Nature",
                    "facing": "down",
                    "dialogue": {
                        "speaker": "Nature Spirit",
                        "portrait": "Nature",
                        "lines": ["Hello."]
                    }
                }
            ]
        }"#;

        let map: MapData = serde_json::from_str(json).expect("map JSON without face_index should still parse");
        assert_eq!(map.npcs[0].dialogue.face_index, 0);
    }

    #[test]
    fn map_data_exits_defaults_to_empty_when_absent() {
        // Older/hand-written map JSON without an "exits" key must still
        // parse (backward compatible), just with no portals.
        let json = r#"{
            "name": "Test Map",
            "width": 10,
            "height": 10,
            "tiles": [],
            "npcs": []
        }"#;

        let map: MapData = serde_json::from_str(json).expect("map JSON without exits should still parse");
        assert!(map.exits.is_empty());
    }
}
