#!/usr/bin/env python3
"""
One-time converter: RPGMaker MZ data → Clean game data format
Converts Map*.json files to simplified map data for Bevy game
"""

import json
import sys
from pathlib import Path

def convert_direction(rpgmaker_dir):
    """Convert RPGMaker direction (2/4/6/8) to simple name"""
    return {
        2: "down",
        4: "left",
        6: "right",
        8: "up"
    }.get(rpgmaker_dir, "down")

def extract_dialogue_from_commands(commands):
    """Extract speaker, portrait, and dialogue lines from event commands"""
    portrait = ""
    lines = []

    for cmd in commands:
        # Code 101 = Show Face (portrait)
        if cmd['code'] == 101 and cmd['parameters']:
            portrait = cmd['parameters'][0]  # Face image name

        # Code 401 = Show Text (dialogue line)
        elif cmd['code'] == 401 and cmd['parameters']:
            lines.append(cmd['parameters'][0])

    return portrait, lines

def simplify_tile_id(tile_id):
    """Convert RPGMaker tile ID to simple 0-based index"""
    if tile_id >= 2048:
        return tile_id - 2048
    elif tile_id >= 1536:
        return tile_id - 1536
    else:
        return 0

def convert_map(rpgmaker_map_path, output_path):
    """Convert a single RPGMaker map to clean format"""

    with open(rpgmaker_map_path, 'r', encoding='utf-8') as f:
        rpg_data = json.load(f)

    width = rpg_data['width']
    height = rpg_data['height']

    # Convert tile data (use layer 0 only for now)
    layer_size = width * height
    raw_tiles = rpg_data['data'][:layer_size]
    tiles = [simplify_tile_id(tile_id) for tile_id in raw_tiles]

    # Extract NPCs from events
    npcs = []
    for event in rpg_data['events']:
        if event is None:
            continue

        # Skip events without pages or graphics
        if not event['pages'] or not event['pages'][0]['image']['characterName']:
            continue

        page = event['pages'][0]
        image = page['image']

        # Extract dialogue
        portrait, lines = extract_dialogue_from_commands(page['list'])

        # Skip NPCs without dialogue
        if not lines:
            continue

        npc = {
            "name": event['name'],
            "x": event['x'],
            "y": event['y'],
            "sprite": image['characterName'],
            "facing": convert_direction(image['direction']),
            "dialogue": {
                "speaker": event['name'],  # Use event name as speaker
                "portrait": portrait,
                "lines": lines
            }
        }
        npcs.append(npc)

    # Build clean output format
    clean_data = {
        "name": rpg_data['displayName'],
        "width": width,
        "height": height,
        "tiles": tiles,
        "npcs": npcs
    }

    # Write to output
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(clean_data, f, indent=2, ensure_ascii=False)

    print(f"✓ Converted {rpgmaker_map_path.name}")
    print(f"  → {output_path}")
    print(f"  Map: {clean_data['name']} ({width}x{height})")
    print(f"  NPCs: {len(npcs)}")
    for npc in npcs:
        print(f"    - {npc['name']} at ({npc['x']}, {npc['y']}) - {len(npc['dialogue']['lines'])} lines")
    print()

def main():
    # Paths
    rpgmaker_data_dir = Path("/home/atobey/src/endgame-of-sre-rpgmaker-mz/data")
    output_dir = Path("/home/atobey/src/sregame/assets/data/maps")

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    # Convert maps
    maps_to_convert = [
        ("Map002.json", "town_of_endgame.json"),  # Hub
        ("Map004.json", "team_marathon.json"),     # Team Marathon interior
    ]

    print("Converting RPGMaker maps to clean format...\n")

    for rpg_file, clean_file in maps_to_convert:
        rpg_path = rpgmaker_data_dir / rpg_file
        out_path = output_dir / clean_file

        if not rpg_path.exists():
            print(f"⚠ Skipping {rpg_file} (not found)")
            continue

        convert_map(rpg_path, out_path)

    print("Conversion complete!")
    print(f"Output files written to: {output_dir}")

if __name__ == "__main__":
    main()
