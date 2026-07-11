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

def clean_dialogue_text(text):
    """Clean up RPGMaker dialogue formatting"""
    # Remove <WordWrap> tags
    text = text.replace('<WordWrap>', '')
    # Remove other common RPGMaker tags if present
    text = text.replace('<br>', ' ')
    return text.strip()

def extract_dialogue_from_commands(commands):
    """Extract speaker, portrait, and dialogue lines from event commands"""
    portrait = ""
    raw_lines = []

    for cmd in commands:
        # Code 101 = Show Face (portrait)
        if cmd['code'] == 101 and cmd['parameters']:
            portrait = cmd['parameters'][0]  # Face image name

        # Code 401 = Show Text (dialogue line)
        elif cmd['code'] == 401 and cmd['parameters']:
            raw_lines.append(cmd['parameters'][0])

    # Clean and intelligently join lines
    if not raw_lines:
        return portrait, []

    lines = []
    current_paragraph = []

    for line in raw_lines:
        cleaned = clean_dialogue_text(line)
        if not cleaned:
            continue

        # Check if this line seems like it was artificially split
        # (doesn't end with punctuation, is short)
        if current_paragraph:
            last_line = current_paragraph[-1]
            # If previous line doesn't end with sentence-ending punctuation, join it
            if not last_line.rstrip().endswith(('.', '!', '?', '"', "'")):
                current_paragraph.append(cleaned)
            else:
                # Previous line was complete, start new paragraph
                lines.append(' '.join(current_paragraph))
                current_paragraph = [cleaned]
        else:
            current_paragraph.append(cleaned)

    # Add final paragraph
    if current_paragraph:
        lines.append(' '.join(current_paragraph))

    return portrait, lines

def simplify_tile_id(tile_id):
    """Convert RPGMaker tile ID to simple 0-based index"""
    if tile_id >= 2048:
        return tile_id - 2048
    elif tile_id >= 1536:
        return tile_id - 1536
    else:
        return 0

# RPGMaker MZ map ID -> our Scene enum variant name (see MapInfos.json).
# Map001 (debug) and Map008 (The War Room) are intentionally omitted: both
# are empty/unused in the original game and have no corresponding Scene.
MAP_ID_TO_SCENE = {
    2: "TownOfEndgame",
    3: "End",
    4: "TeamMarathon",
    5: "TeamDisco",
    6: "TeamInferno",
    7: "MahoganyRow",
    9: "TeamMarathonRetro",
    10: "Intro",
}

def extract_exits_from_events(events):
    """Extract Transfer Player (code 201) events as map exits.

    RPGMaker MZ code-201 "Transfer Player" parameters are, in order:
      [designation, mapId, x, y, direction, fadeType]
    where designation 0 = direct map/x/y (the only form used anywhere in
    this game's data - verified by scanning every Map*.json), 1 = designation
    via variables (unsupported here; we skip and warn rather than guess).

    The exit's trigger position is the *source* event's own x/y in the map
    it's defined in (i.e. the door tile itself), not a command parameter.
    """
    exits = []

    for event in events:
        if event is None:
            continue

        for page in event['pages']:
            for cmd in page['list']:
                if cmd['code'] != 201:
                    continue

                params = cmd['parameters']
                if len(params) != 6:
                    raise ValueError(
                        f"Unexpected Transfer Player parameter count in event "
                        f"'{event['name']}': {params}"
                    )
                designation, map_id, target_x, target_y, _direction, _fade_type = params

                if designation != 0:
                    print(f"  ⚠ Skipping variable-designated transfer in event "
                          f"'{event['name']}' (unsupported)")
                    continue

                target_scene = MAP_ID_TO_SCENE.get(map_id)
                if target_scene is None:
                    print(f"  ⚠ Skipping transfer to unmapped mapId {map_id} "
                          f"in event '{event['name']}'")
                    continue

                exits.append({
                    "trigger_x": event['x'],
                    "trigger_y": event['y'],
                    "target_scene": target_scene,
                    "target_spawn_x": target_x,
                    "target_spawn_y": target_y,
                })

    return exits

def convert_map(rpgmaker_map_path, output_path, extra_exits=None):
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

    # Extract portal/door triggers from Transfer Player events
    exits = extract_exits_from_events(rpg_data['events'])
    if extra_exits:
        exits.extend(extra_exits)

    # Build clean output format
    clean_data = {
        "name": rpg_data['displayName'],
        "width": width,
        "height": height,
        "tiles": tiles,
        "npcs": npcs,
        "exits": exits
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
    print(f"  Exits: {len(exits)}")
    for exit_data in exits:
        print(f"    - ({exit_data['trigger_x']}, {exit_data['trigger_y']}) -> "
              f"{exit_data['target_scene']} @ ({exit_data['target_spawn_x']}, "
              f"{exit_data['target_spawn_y']})")
    print()

# NOTE: the real game has no simple walk-into-door (code 201) event from
# Town of Endgame into Team Marathon (mapId 4) anywhere in the RPGMaker data -
# verified by scanning every Map*.json for a code-201 command targeting
# mapId 4; there is none. The only door out of Map002 with "Team Marathon" in
# its labeling is event "To Inn" (a comment reads `<Label: Team Marathon>`),
# but it actually transfers to mapId 9 ("Team Marathon - Retro"), not mapId 4.
# The real mechanism for reaching mapId 4 is CommonEvents.json event #12
# ("Crystal Main"), a menu-driven, multi-destination warp (its Team Marathon
# branch targets x=12, y=15) - not a simple map door, and out of scope for
# this phase's code-201 extractor.
#
# Phase 1a (the generic Scene/spawn_map/transitions scaffolding) needs one
# working, bidirectional Town <-> Team Marathon door pair to validate the new
# transition system end-to-end. Until a proper "team select" menu (mirroring
# Crystal Main) is designed, this hand-authored placeholder stands in for it:
# trigger tile chosen clear of existing doors/NPCs/player spawn in Town of
# Endgame; target spawn coordinates reused from the real Crystal Main data
# for authenticity. TODO(later phase): replace with a real in-fiction trigger
# (or an actual team-select menu) once that mechanism is designed.
TOWN_TO_TEAM_MARATHON_PLACEHOLDER = [{
    "trigger_x": 30,
    "trigger_y": 30,
    "target_scene": "TeamMarathon",
    "target_spawn_x": 12,
    "target_spawn_y": 15,
}]

def main():
    # Paths. Output is resolved relative to this script's location (not a
    # hardcoded absolute path) so this converter works correctly regardless
    # of which git worktree/clone it's run from.
    rpgmaker_data_dir = Path("/home/atobey/src/endgame-of-sre-rpgmaker-mz/data")
    output_dir = Path(__file__).resolve().parent.parent / "assets" / "data" / "maps"

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    # Convert maps
    maps_to_convert = [
        ("Map002.json", "town_of_endgame.json", TOWN_TO_TEAM_MARATHON_PLACEHOLDER),  # Hub
        ("Map004.json", "team_marathon.json", None),  # Team Marathon interior
    ]

    print("Converting RPGMaker maps to clean format...\n")

    for rpg_file, clean_file, extra_exits in maps_to_convert:
        rpg_path = rpgmaker_data_dir / rpg_file
        out_path = output_dir / clean_file

        if not rpg_path.exists():
            print(f"⚠ Skipping {rpg_file} (not found)")
            continue

        convert_map(rpg_path, out_path, extra_exits=extra_exits)

    print("Conversion complete!")
    print(f"Output files written to: {output_dir}")

if __name__ == "__main__":
    main()
