#!/usr/bin/env python3
"""
One-time converter: RPGMaker MZ data -> Clean game data format.

Converts Map*.json files (+ their tileset sheets) to:
  - a simplified map data JSON for the Bevy game (tiles/upper_tiles/collision/npcs/exits)
  - a single flattened, fully-composited PNG atlas per map

Tile IDs are decoded and composited using the exact same autotile algorithm
RPGMaker MZ's own engine uses (see tools/rpgmaker_tiles.py for the ported
algorithm + source references). All RPGMaker-specific complexity (autotile
shape tables, tileset flag bits, z-layer compositing order) lives here at
build time; the Rust/Bevy side only ever consumes plain atlas indices.
"""

import json
from pathlib import Path

from PIL import Image

from rpgmaker_tiles import (
    TILE_SIZE,
    apply_shadow,
    is_fully_blocked,
    is_higher_tile,
    is_shadowing_tile,
    is_table_tile,
    load_tileset_sheets,
    render_table_edge_overlay,
    render_tile,
)

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
RPGMAKER_ROOT = Path("/home/atobey/src/endgame-of-sre-rpgmaker-mz")
RPGMAKER_DATA_DIR = RPGMAKER_ROOT / "data"
RPGMAKER_TILESETS_DIR = RPGMAKER_ROOT / "img" / "tilesets"

# Resolve output paths relative to this script's location (tools/../..), so
# this always writes into whichever checkout/worktree it's run from rather
# than a hardcoded path to a specific clone.
REPO_ROOT = Path(__file__).resolve().parent.parent
OUTPUT_MAPS_DIR = REPO_ROOT / "assets" / "data" / "maps"
OUTPUT_TILESETS_DIR = REPO_ROOT / "assets" / "textures" / "tilesets"

ATLAS_COLUMNS = 16

# Number of z-layer "planes" RPGMaker MZ stores per map cell: 4 tile-graphic
# layers + 1 shadow-pen layer + 1 region-ID layer. Confirmed empirically
# against Map002.json/Map004.json (data.length == width*height*6) and
# against rmmz_core.js's _readMapData/_addSpot (only z=0..4 are ever read
# for rendering; z=5 is the region ID, unused here).
EXPECTED_DATA_PLANES = 6

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


# ---------------------------------------------------------------------------
# Dialogue/NPC extraction
# ---------------------------------------------------------------------------
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
    text = text.replace('<WordWrap>', '')
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

    if not raw_lines:
        return portrait, []

    lines = []
    current_paragraph = []

    for line in raw_lines:
        cleaned = clean_dialogue_text(line)
        if not cleaned:
            continue

        if current_paragraph:
            last_line = current_paragraph[-1]
            if not last_line.rstrip().endswith(('.', '!', '?', '"', "'")):
                current_paragraph.append(cleaned)
            else:
                lines.append(' '.join(current_paragraph))
                current_paragraph = [cleaned]
        else:
            current_paragraph.append(cleaned)

    if current_paragraph:
        lines.append(' '.join(current_paragraph))

    return portrait, lines


def extract_npcs(rpg_data):
    npcs = []
    for event in rpg_data['events']:
        if event is None:
            continue

        if not event['pages'] or not event['pages'][0]['image']['characterName']:
            continue

        page = event['pages'][0]
        image = page['image']

        portrait, lines = extract_dialogue_from_commands(page['list'])

        if not lines:
            continue

        npcs.append({
            "name": event['name'],
            "x": event['x'],
            "y": event['y'],
            "sprite": image['characterName'],
            "facing": convert_direction(image['direction']),
            "dialogue": {
                "speaker": event['name'],
                "portrait": portrait,
                "lines": lines
            }
        })
    return npcs


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
                    print(f"  Skipping variable-designated transfer in event "
                          f"'{event['name']}' (unsupported)")
                    continue

                target_scene = MAP_ID_TO_SCENE.get(map_id)
                if target_scene is None:
                    print(f"  Skipping transfer to unmapped mapId {map_id} "
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


# ---------------------------------------------------------------------------
# Tile compositing / atlas building
# ---------------------------------------------------------------------------
def get_map_plane(data, width, height, z, x, y):
    """Mirrors Tilemap._readMapData (rmmz_core.js lines 2618-2636) for our
    maps, all of which have scrollType=0 (no wrap), so out-of-range reads
    simply return 0 rather than wrapping."""
    if x < 0 or x >= width or y < 0 or y >= height:
        return 0
    return data[(z * height + y) * width + x]


class TileCompositor:
    """Composites RPGMaker map cells into a deduplicated 48x48 tile atlas.

    Index 0 is always reserved for a fully-transparent blank tile, used by
    `upper_tiles` for "no upper-layer decoration here" and as a safe
    fallback if a ground cell ever composites to nothing.
    """

    def __init__(self, flags, sheets):
        self.flags = flags
        self.sheets = sheets
        blank = Image.new("RGBA", (TILE_SIZE, TILE_SIZE), (0, 0, 0, 0))
        self.atlas_images = [blank]
        self._index_by_bytes = {blank.tobytes(): 0}

    def _register(self, image):
        key = image.tobytes()
        index = self._index_by_bytes.get(key)
        if index is None:
            index = len(self.atlas_images)
            self.atlas_images.append(image)
            self._index_by_bytes[key] = index
        return index

    def composite_cell(self, tile_id0, tile_id1, tile_id2, tile_id3, shadow_bits, upper_tile_id1):
        """Mirrors Tilemap._addSpot (rmmz_core.js lines 2436-2463), with
        _isOverpassPosition hardcoded to false (as it always is in the
        base engine -- see rmmz_core.js line 2646-2648), splitting each of
        the 4 tile-graphic layers into a "ground" bucket and an "upper"
        bucket by that tile's own 0x10 flag bit, in RPGMaker's draw order:
        layer0, layer1, [shadow, table-edge], layer2, layer3."""
        ground = Image.new("RGBA", (TILE_SIZE, TILE_SIZE), (0, 0, 0, 0))
        upper = Image.new("RGBA", (TILE_SIZE, TILE_SIZE), (0, 0, 0, 0))

        def add_spot_tile(tile_id):
            image = render_tile(tile_id, self.sheets, self.flags)
            if image is None:
                return
            target = upper if is_higher_tile(self.flags, tile_id) else ground
            target.alpha_composite(image)

        add_spot_tile(tile_id0)
        add_spot_tile(tile_id1)
        apply_shadow(ground, shadow_bits)
        if (
            is_table_tile(self.flags, upper_tile_id1)
            and not is_table_tile(self.flags, tile_id1)
            and not is_shadowing_tile(tile_id0)
        ):
            edge = render_table_edge_overlay(upper_tile_id1, self.sheets)
            if edge is not None:
                ground.alpha_composite(edge)
        add_spot_tile(tile_id2)
        add_spot_tile(tile_id3)

        return self._register(ground), self._register(upper)

    def build_atlas_image(self):
        cols = ATLAS_COLUMNS
        rows = (len(self.atlas_images) + cols - 1) // cols
        atlas = Image.new("RGBA", (cols * TILE_SIZE, rows * TILE_SIZE), (0, 0, 0, 0))
        for index, tile_image in enumerate(self.atlas_images):
            x = (index % cols) * TILE_SIZE
            y = (index // cols) * TILE_SIZE
            atlas.paste(tile_image, (x, y))
        return atlas


def convert_tiles_and_collision(rpg_data, tileset_entry, compositor):
    """Bake one map's tiles/upper_tiles/collision using the given
    (possibly shared) TileCompositor. Callers that pass the *same*
    compositor instance across multiple maps get all of those maps'
    distinct tiles deduplicated into one shared atlas - required whenever
    more than one map is meant to share an output atlas file/tileset_key
    (see build_tileset_group in main()); passing a fresh compositor per
    map would silently make each map's baked tile indices only valid
    against its own atlas, while every map after the first overwrites
    that atlas file on disk with its own - a real bug we hit and fixed
    while wiring up Team Marathon + Team Marathon Retro, both of which
    share the "inside_tileset" key.
    """
    width = rpg_data['width']
    height = rpg_data['height']
    data = rpg_data['data']
    num_cells = width * height

    if num_cells == 0 or len(data) % num_cells != 0:
        raise ValueError(
            f"map data length {len(data)} is not a multiple of "
            f"{width}x{height}={num_cells}"
        )
    num_planes = len(data) // num_cells
    if num_planes != EXPECTED_DATA_PLANES:
        raise ValueError(
            f"expected {EXPECTED_DATA_PLANES} data planes per RPG Maker MZ "
            f"map cell (4 tile layers + shadow + region), got {num_planes}; "
            f"the compositor has only been validated for the 6-plane case"
        )

    flags = tileset_entry['flags']
    if len(flags) != 8192:
        raise ValueError(
            f"expected 8192 tileset flags (tilesets[id]['flags']), got {len(flags)}"
        )

    tiles = [0] * num_cells
    upper_tiles = [0] * num_cells
    collision = [False] * num_cells

    for y in range(height):
        for x in range(width):
            index = y * width + x
            tile_id0 = get_map_plane(data, width, height, 0, x, y)
            tile_id1 = get_map_plane(data, width, height, 1, x, y)
            tile_id2 = get_map_plane(data, width, height, 2, x, y)
            tile_id3 = get_map_plane(data, width, height, 3, x, y)
            shadow_bits = get_map_plane(data, width, height, 4, x, y)
            upper_tile_id1 = get_map_plane(data, width, height, 1, x, y - 1)

            ground_index, upper_index = compositor.composite_cell(
                tile_id0, tile_id1, tile_id2, tile_id3, shadow_bits, upper_tile_id1
            )
            tiles[index] = ground_index
            upper_tiles[index] = upper_index

            # Flag-based collision: blocked if any of this cell's 4
            # tile-graphic layers is impassable from all 4 directions.
            #
            # This is deliberately OR'd with "cell sits on the map's outer
            # edge", because we found (empirically, on Town of Endgame's
            # left/right border) that RPGMaker map borders are sometimes
            # drawn using *directional* passage flags rather than the
            # full 0x0F (e.g. tile 6872, the left-column wall, has flags
            # 0x0e04 -- only its right-facing edge is marked impassable,
            # which is sufficient in the real engine's per-direction
            # movement check but invisible to our simpler "fully blocked"
            # heuristic). Directional passability is explicitly out of
            # scope, so instead of implementing it we just guarantee the
            # map's outer ring is always solid, which matches every map
            # we've inspected and is a strict improvement over silently
            # letting the player walk into/along border wall graphics.
            collision[index] = (
                any(
                    is_fully_blocked(flags, tile_id)
                    for tile_id in (tile_id0, tile_id1, tile_id2, tile_id3)
                )
                or x == 0 or x == width - 1 or y == 0 or y == height - 1
            )

    stats = {
        "blocked_cells": sum(collision),
        "upper_cells": sum(1 for v in upper_tiles if v != 0),
    }
    return tiles, upper_tiles, collision, stats


# ---------------------------------------------------------------------------
# Orchestration
# ---------------------------------------------------------------------------
def load_tilesets_json():
    with open(RPGMAKER_DATA_DIR / "Tilesets.json", encoding="utf-8") as f:
        return json.load(f)


def convert_map(rpgmaker_map_path, output_path, tileset_entry, compositor):
    """Convert a single RPGMaker map to clean format, baking its tiles into
    the given (possibly shared) compositor. Does NOT write the atlas image -
    callers sharing one compositor across multiple maps must save the atlas
    themselves once, after all of those maps have been converted (see
    build_tileset_group)."""

    with open(rpgmaker_map_path, 'r', encoding='utf-8') as f:
        rpg_data = json.load(f)

    width = rpg_data['width']
    height = rpg_data['height']
    tileset_id = rpg_data['tilesetId']

    tiles, upper_tiles, collision, stats = convert_tiles_and_collision(
        rpg_data, tileset_entry, compositor
    )

    npcs = extract_npcs(rpg_data)
    exits = extract_exits_from_events(rpg_data['events'])

    clean_data = {
        "name": rpg_data['displayName'],
        "width": width,
        "height": height,
        "tiles": tiles,
        "upper_tiles": upper_tiles,
        "collision": collision,
        "npcs": npcs,
        "exits": exits,
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(clean_data, f, indent=2, ensure_ascii=False)

    print(f"Converted {rpgmaker_map_path.name}")
    print(f"  -> {output_path}")
    print(
        f"  Map: {clean_data['name']} ({width}x{height}), "
        f"tileset {tileset_id} ({tileset_entry['name']})"
    )
    print(
        f"  Blocked cells: {stats['blocked_cells']}/{width * height}  "
        f"Upper-layer cells: {stats['upper_cells']}"
    )
    print(f"  NPCs: {len(npcs)}")
    for npc in npcs:
        print(f"    - {npc['name']} at ({npc['x']}, {npc['y']}) - {len(npc['dialogue']['lines'])} lines")
    print(f"  Exits: {len(exits)}")
    for exit_data in exits:
        print(f"    - ({exit_data['trigger_x']}, {exit_data['trigger_y']}) -> "
              f"{exit_data['target_scene']} @ ({exit_data['target_spawn_x']}, "
              f"{exit_data['target_spawn_y']})")
    print()


def build_tileset_group(entries, tilesets_json, atlas_output_path):
    """Convert every map in `entries` (a list of (rpg_file, clean_file)
    pairs) into one *shared* atlas at `atlas_output_path`, deduplicating
    composited tiles across all of them via one TileCompositor instance.

    This matters whenever more than one map is meant to share an output
    atlas/tileset_key (e.g. "inside_tileset" is used by every interior
    scene, see tilemap.rs::scene_config): giving each map its own
    from-scratch compositor would make each map's baked tile indices valid
    only against its own atlas, while whichever map converts last would
    silently overwrite the shared atlas file on disk with just its own
    tiles - corrupting every other map's indices without any error. This
    was a real bug caught while wiring up Team Marathon + Team Marathon
    Retro, both "inside_tileset".
    """
    if not entries:
        return

    first_rpg_file = entries[0][0]
    with open(RPGMAKER_DATA_DIR / first_rpg_file, encoding="utf-8") as f:
        tileset_id = json.load(f)['tilesetId']
    tileset_entry = tilesets_json[tileset_id]
    if tileset_entry is None:
        raise ValueError(f"tileset id {tileset_id} not present in Tilesets.json")

    flags = tileset_entry['flags']
    sheets = load_tileset_sheets(tileset_entry['tilesetNames'], RPGMAKER_TILESETS_DIR)
    compositor = TileCompositor(flags, sheets)

    for rpg_file, clean_file in entries:
        rpg_path = RPGMAKER_DATA_DIR / rpg_file
        if not rpg_path.exists():
            print(f"Skipping {rpg_file} (not found)")
            continue
        convert_map(rpg_path, OUTPUT_MAPS_DIR / clean_file, tileset_entry, compositor)

    atlas_image = compositor.build_atlas_image()
    atlas_output_path.parent.mkdir(parents=True, exist_ok=True)
    atlas_image.save(atlas_output_path)
    print(
        f"Wrote shared atlas {atlas_output_path} "
        f"({atlas_image.width}x{atlas_image.height}px, "
        f"{len(compositor.atlas_images)} unique composited tiles across "
        f"{len(entries)} map(s))\n"
    )


# NOTE on Team Marathon (mapId 4) vs Team Marathon - Retro (mapId 9): the
# real game has no simple walk-into-door (code 201) event from Town of
# Endgame into mapId 4 anywhere in the RPGMaker data - verified by scanning
# every Map*.json for a code-201 command targeting mapId 4; there is none.
# The only door out of Map002 themed around "Team Marathon" is event "To
# Inn", which actually transfers to mapId 9 ("Team Marathon - Retro"), not
# mapId 4. The real mechanism for reaching mapId 4 is CommonEvents.json
# event #12 ("Crystal Main"), a menu-driven, multi-destination dev/debug
# warp - not a simple map door. Per product decision, mapId 9 (Retro) is
# the canonical, door-connected "Team Marathon" location (it's also the
# game's actual final content state); mapId 4 is converted too (it's free,
# and may be useful bonus/optional content later) but nothing links to it
# from Town, matching the original's shipped topology.
def main():
    tilesets_json = load_tilesets_json()

    # Maps sharing an atlas file (tileset_key) must be converted together
    # via build_tileset_group so their tiles dedupe into one shared atlas -
    # see its docstring. (rpg file, clean output json) pairs per group.
    tileset_groups = [
        ("town_tileset.png", [
            ("Map002.json", "town_of_endgame.json"),  # Hub (Outside tileset)
        ]),
        ("inside_tileset.png", [
            ("Map004.json", "team_marathon.json"),        # base (not door-connected; see note above)
            ("Map009.json", "team_marathon_retro.json"),   # Retro (the real, door-connected location)
        ]),
    ]

    print("Converting RPGMaker maps to clean format...\n")

    for atlas_file, entries in tileset_groups:
        build_tileset_group(entries, tilesets_json, OUTPUT_TILESETS_DIR / atlas_file)

    print("Conversion complete!")
    print(f"Map output: {OUTPUT_MAPS_DIR}")
    print(f"Atlas output: {OUTPUT_TILESETS_DIR}")


if __name__ == "__main__":
    main()
