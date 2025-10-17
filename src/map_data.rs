use bevy::prelude::*;
use serde::Deserialize;
use anyhow::{Context, Result};
use std::fs;

#[derive(Debug, Deserialize)]
pub struct MapData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<u32>,
    pub npcs: Vec<NpcData>,
}

#[derive(Debug, Deserialize)]
pub struct NpcData {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub sprite: String,
    pub facing: String,
    pub dialogue: DialogueData,
}

#[derive(Debug, Deserialize)]
pub struct DialogueData {
    pub speaker: String,
    pub portrait: String,
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

pub fn tile_to_world(tile_x: u32, tile_y: u32, map_width: u32, map_height: u32) -> Vec2 {
    const TILE_SIZE: f32 = 48.0;

    let world_x = (tile_x as f32 - map_width as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;
    let world_y = (tile_y as f32 - map_height as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0;

    Vec2::new(world_x, world_y)
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
