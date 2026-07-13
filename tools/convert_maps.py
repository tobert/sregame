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
from collections import Counter
from pathlib import Path

from PIL import Image

from rpgmaker_tiles import (
    SET_NUMBER_B,
    TILE_SIZE,
    TILE_ID_A1,
    TILE_ID_A4,
    apply_shadow,
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


def _majority_vote(values, fallback):
    """Pick the most frequent value in `values`, breaking ties by first
    occurrence (deterministic), or `fallback` if `values` is empty."""
    if not values:
        return fallback
    counts = Counter(values)
    best_count = max(counts.values())
    for value in values:
        if counts[value] == best_count:
            return value
    return fallback  # unreachable: values is non-empty


def extract_dialogue_from_commands(commands, fallback_speaker, keep_lines_separate=False):
    """Extract speaker, portrait, and dialogue lines from event commands.

    Segmentation is keyed on code-101 "Show Face" boundaries, *not* on
    sentence-ending punctuation (an earlier version of this function joined
    consecutive code-401 "Show Text" lines into one paragraph until it saw a
    line ending in one of . ! ? " ', which happened to work for most of this
    game's prose but is not how RPGMaker MZ actually groups dialogue: each
    code-101 call starts a new message box, and every code-401 line up to
    the next code-101 belongs to that same box). Verified against the
    already-shipped assets/data/maps/town_of_endgame.json: the old
    punctuation heuristic silently mis-segmented several real NPCs there
    (e.g. "Nanny Ogg Vorbis", whose second code-101 box is actually a
    one-line aside from a *different* speaker, "Amy" - the old code glued
    it onto Nanny Ogg's own line as one paragraph under Nanny Ogg's
    portrait; e.g. "Courage" and "Frau Barella", where a genuine two-line
    word-wrapped box got wrongly split into two paragraphs just because its
    first line happened to end in a period).

    Each code-101 box's parameters are, in order, [faceName, faceIndex,
    background, position, speakerName] - the last (a VisuStella
    MessageCore name-box override) being the name actually displayed above
    the text in-game, distinct from, and sometimes quite different from,
    the event's own RPGMaker-editor name (e.g. Map002's event "Alls
    Johnpaw" displays as "Paws Alljohn"; Map003's "Doctor Mcfire" displays
    as "Doctor McFire"). faceIndex (0-7) selects which cell of the
    faceName sheet to show - RPGMaker MZ face sheets are a fixed 4-column
    grid of 144x144px cells (see ImageManager.faceWidth/faceHeight in
    rmmz_managers.js and Window_Base.prototype.drawFace in
    rmmz_windows.js, both hardcoded regardless of a given sheet's actual
    pixel dimensions), so faceIndex 0-7 spans a 4x2 grid. A single NPC
    event can also contain more than one code-101 box using more than one
    face/index/name - most often because Amy (the player) briefly
    interjects a one-line aside into an NPC's own conversation. Our
    NpcData/DialogueData format has only one speaker/portrait/face_index
    per NPC interaction, so the effective speaker name, portrait, and face
    index are all resolved via majority vote across all of this event's
    code-101 boxes (falling back to `fallback_speaker`, normally the
    event's own editor name, if no box ever set a name-box override, or to
    face_index 0 if no box set a portrait) rather than "whichever code-101
    happened to run last" - the previous behavior, which for e.g. Map003's
    "Dave" (3 boxes as Dave, 1 one-line comeback as Amy) showed *Amy's*
    portrait for the entire 4-line interaction because hers was the last
    code-101 seen.

    `keep_lines_separate`, when set, keeps every code-401 line in a box as
    its own entry in the returned `lines` list instead of joining them with
    spaces into one paragraph. The default (join) is correct for ordinary
    word-wrapped prose, where a box's lines are fragments of one sentence
    reflowed to fit the message window. It is wrong for the two "title
    card" events this game uses for its opening/closing credits (Map010
    event 3 and Map003 event 3): one code-101 box containing three short,
    independent statements (a title, a byline, a role/credit line) with no
    word-wrap tags and no sentence-ending punctuation to join on - joining
    them produces one garbled run-on string. See EVENT_OVERRIDES.
    """
    groups = []
    current = None

    for cmd in commands:
        # Code 101 = Show Face: starts a new message box.
        if cmd['code'] == 101 and cmd['parameters']:
            params = cmd['parameters']
            current = {
                "portrait": params[0] if params[0] else "",
                "face_index": params[1] if len(params) >= 2 else 0,
                "speaker": params[4] if len(params) >= 5 and params[4] else "",
                "raw_lines": [],
            }
            groups.append(current)

        # Code 401 = Show Text: one line within the current box.
        elif cmd['code'] == 401 and cmd['parameters']:
            if current is None:
                # Defensive: no game data we've seen has a bare Show Text
                # with no preceding Show Face, but don't silently drop the
                # line if it ever happens.
                current = {"portrait": "", "speaker": "", "raw_lines": []}
                groups.append(current)
            current['raw_lines'].append(cmd['parameters'][0])

    lines = []
    portraits = []
    face_indices = []
    speakers = []

    for group in groups:
        cleaned = [clean_dialogue_text(raw) for raw in group['raw_lines']]
        cleaned = [c for c in cleaned if c]
        if not cleaned:
            continue

        if keep_lines_separate:
            lines.extend(cleaned)
        else:
            lines.append(' '.join(cleaned))

        if group['portrait']:
            portraits.append(group['portrait'])
            # Paired with portrait, not tracked independently: a faceIndex
            # is meaningless without knowing which sheet (portrait) it
            # indexes into.
            face_indices.append(group['face_index'])
        if group['speaker']:
            speakers.append(group['speaker'])

    speaker = _majority_vote(speakers, fallback_speaker)
    portrait = _majority_vote(portraits, "")
    face_index = _majority_vote(face_indices, 0)

    return speaker, portrait, face_index, lines


# Per-event overrides keyed by (source RPGMaker filename, event id), for the
# rare cases that don't fit the generic extraction rules above. Keying by
# filename+id (rather than inferring these structurally, e.g. "no sprite" or
# "no sentence punctuation") means an override can never accidentally
# misfire on some other, unrelated event.
#
# Map010 event 3 and Map003 event 3 are the *same* title/credits-card
# content (RPGMaker's default "EV003" name for an unnamed event, in both
# maps): one code-101 "Show Face" (portrait Amy) followed by three short,
# independent lines (a title, a byline, a role/credit line) - not a
# word-wrapped continuation of one sentence, so both need
# keep_lines_separate (see extract_dialogue_from_commands) to avoid being
# joined into one garbled run-on string.
#
#   - Map010 event 3 is the game's opening title card. In the source,
#     image.characterName is empty - RPGMaker plays it as a scripted/
#     autorun message with no overworld sprite at all, a different
#     interaction paradigm (auto-display on map load) than every other
#     piece of dialogue in this game (walk up + press E). Building a whole
#     separate "autorun event" system in the Rust/Bevy side for this one
#     title card would be real architecture scope creep for a single use,
#     so we deliberately synthesize it as an ordinary walk-up NPC here at
#     conversion time instead: reusing the player's own "Amy-Walking"
#     sprite (it's Amy speaking) and "Amy" as both portrait and speaker
#     name (the source event has no code-101 name-box override to fall
#     back on, only an empty string). This is the mechanism by which that
#     synthetic NPC survives a future re-run of this script rather than
#     being a one-off hand-edit of intro.json that a re-conversion would
#     silently discard.
#   - Map003 event 3 is the closing credits version of the same content
#     (its third line is "Assets by VisuStella" instead of a job title).
#     It already has a real sprite ("Nature") in the source data, so it
#     needs no sprite synthesis - just the same line-splitting override.
EVENT_OVERRIDES = {
    ("Map010.json", 3): {
        "synthetic_sprite": "Amy-Walking",
        "synthetic_speaker": "Amy",
        "keep_lines_separate": True,
    },
    ("Map003.json", 3): {
        "keep_lines_separate": True,
    },
    # doggo (town). In the source he's a dialogue-less parallel event running
    # a scripted left/right patrol route. Promoting him to a real NPC is a
    # deliberate content addition (Amy, 2026-07-12): he wanders RANDOMLY
    # (not the original patrol - also her call) and barks when talked to.
    # "through" carries the source page's through=true (he never blocks);
    # wandering still respects map passability so he doesn't stroll into
    # the pond - engine-divergent for a through character, intent-faithful.
    ("Map002.json", 13): {
        "synthetic_dialogue": ["wan wan!"],
        "synthetic_speaker": "doggo",
        "wander": True,
        "through": True,
    },
}


def extract_npcs(rpg_data, source_filename=""):
    npcs = []
    for event in rpg_data['events']:
        if event is None:
            continue

        if not event['pages']:
            continue

        page = event['pages'][0]
        image = page['image']
        override = EVENT_OVERRIDES.get((source_filename, event['id']), {})

        if not image['characterName'] and 'synthetic_sprite' not in override:
            continue

        speaker, portrait, face_index, lines = extract_dialogue_from_commands(
            page['list'],
            fallback_speaker=event['name'],
            keep_lines_separate=override.get('keep_lines_separate', False),
        )

        # An image event with no dialogue of its own is normally a prop (see
        # extract_props), unless an override grants it synthetic lines -
        # doggo's "wan wan!" is the motivating case.
        if not lines:
            lines = override.get('synthetic_dialogue', [])
        if not lines:
            continue

        if 'synthetic_speaker' in override:
            speaker = override['synthetic_speaker']

        npcs.append({
            "name": event['name'],
            "x": event['x'],
            "y": event['y'],
            "sprite": strip_sheet_prefix(image['characterName']) or override.get('synthetic_sprite'),
            # Which character slot (0-7) of the sheet this event uses; the
            # renderer slices the 4x2-character sheet with this (see
            # src/character_sheet.rs). Dropping it renders every NPC as the
            # sheet's top-left character.
            "sprite_index": image['characterIndex'],
            # RPGMaker "Stepping Animation": the character plays its walk
            # pattern in place while stationary. Nearly every NPC in the
            # original has this on - without it the town is full of statues.
            "step_anime": page['stepAnime'],
            "facing": convert_direction(image['direction']),
            # Random tile-step wandering + RPGMaker's through flag (never
            # blocks the player). Only granted by override for now (doggo);
            # the original's scripted patrol routes are not ported.
            "wander": override.get('wander', False),
            "through": override.get('through', page['through']),
            "dialogue": {
                "speaker": speaker,
                "portrait": portrait,
                "face_index": face_index,
                "lines": lines
            }
        })
    return npcs


def extract_dialogue_segments(commands):
    """Extract a scripted scene as ordered per-box segments, each keeping its
    OWN speaker/portrait (unlike extract_dialogue_from_commands, which
    majority-votes one speaker for an NPC). Used for exit events that play a
    scene before transferring - Map009's "retro dialog" retrospective is 15
    boxes across multiple speakers."""
    segments = []
    current = None

    for cmd in commands:
        if cmd['code'] == 101:
            params = cmd['parameters']
            current = {
                "speaker": params[4] if len(params) > 4 and params[4] else "",
                "portrait": params[0] or "",
                "face_index": params[1],
                "lines": [],
            }
            segments.append(current)
        elif cmd['code'] == 401 and current is not None:
            cleaned = clean_dialogue_text(cmd['parameters'][0])
            if cleaned:
                current['lines'].append(cleaned)

    return [
        {
            "speaker": seg['speaker'],
            "portrait": seg['portrait'],
            "face_index": seg['face_index'],
            "text": ' '.join(seg['lines']),
        }
        for seg in segments
        if seg['lines']
    ]


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
                    # RPGMaker trigger 0 = Action Button (player must press
                    # confirm while on/at the event); 1/2 = touch. Flattening
                    # everything to touch turned Map009's "retro dialog"
                    # event (action-triggered, next to the inn table) into a
                    # warp mine that teleported passers-by to the End scene.
                    "trigger": "action" if page['trigger'] == 0 else "touch",
                    # Scripted scene played BEFORE the transfer fires (the
                    # retro dialog's 15-box retrospective - the game's
                    # climax dialogue, previously dropped entirely).
                    "dialogue": extract_dialogue_segments(page['list']),
                })

    return exits


def extract_props(events, npcs, doors):
    """Extract ambient visual props: events that carry a character image but
    have neither dialogue (those are NPCs) nor a transfer (those are doors).
    Down to one in practice: 'The Boss's Truck' (a static vehicle that
    blocks its tile: RPGMaker events with priority 1 and through=false are
    impassable, and our baked tile collision knows nothing about events, so
    the blocking is carried per-prop). doggo used to be the second prop; an
    EVENT_OVERRIDES entry now promotes him to a wandering NPC.
    """
    npc_positions = {(n['x'], n['y']) for n in npcs}
    door_positions = {(d['x'], d['y']) for d in doors}
    props = []

    for event in events:
        if event is None or not event['pages']:
            continue

        page = event['pages'][0]
        image = page['image']
        if not image['characterName']:
            continue
        if (event['x'], event['y']) in npc_positions or (event['x'], event['y']) in door_positions:
            continue

        sheet_path = RPGMAKER_ROOT / "img" / "characters" / f"{image['characterName']}.png"
        with Image.open(sheet_path) as sheet:
            frame_width = sheet.width // 12
            frame_height = sheet.height // 8

        props.append({
            "name": event['name'],
            "x": event['x'],
            "y": event['y'],
            "sprite": strip_sheet_prefix(image['characterName']),
            "sprite_index": image['characterIndex'],
            "facing": convert_direction(image['direction']),
            "pattern": image['pattern'],
            "step_anime": page['stepAnime'],
            "blocks": page['priorityType'] == 1 and not page['through'],
            "frame_width": frame_width,
            "frame_height": frame_height,
        })

    return props


def strip_sheet_prefix(character_name):
    """RPGMaker filename prefixes ('!' = object sheet: no 6px draw offset,
    ignores bush; '$' = single-character sheet) are metadata, not identity.
    Our copied assets drop them ('!doors.png' ships as 'doors.png'), so
    sprite references in clean JSON must drop them too."""
    return character_name.lstrip('!$')


def extract_doors(events, source_filename=""):
    """Extract visible door sprites: events that both carry a character image
    and perform a code-201 transfer (the town's '!doors' events). The exits
    themselves are extracted separately by extract_exits_from_events; this
    captures only the visual so the renderer can draw (and animate) a door
    on the trigger tile. Interior 'To Town' exits have no image in the
    original and stay invisible - that's faithful, not a gap.

    frame_width/frame_height are baked here because RPGMaker derives frame
    size from sheet dimensions (width/12 x height/8 - '!doors.png' is
    576x768, so doors are 48x96: one tile wide, TWO tiles tall) and the
    runtime shouldn't need to inspect image dimensions before spawning.
    """
    doors = []

    for event in events:
        if event is None or not event['pages']:
            continue

        page = event['pages'][0]
        image = page['image']
        if not image['characterName']:
            continue
        if not any(cmd['code'] == 201 for cmd in page['list']):
            continue

        sheet_path = RPGMAKER_ROOT / "img" / "characters" / f"{image['characterName']}.png"
        with Image.open(sheet_path) as sheet:
            frame_width = sheet.width // 12
            frame_height = sheet.height // 8

        doors.append({
            "x": event['x'],
            "y": event['y'],
            "sprite": strip_sheet_prefix(image['characterName']),
            "sprite_index": image['characterIndex'],
            "facing": convert_direction(image['direction']),
            "pattern": image['pattern'],
            "frame_width": frame_width,
            "frame_height": frame_height,
        })

    return doors


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


# The VisuStella "Outside" B sheet's first two rows are hanging shop signs:
# tiles 1-14 carry icons/text (1 = sword, 4 = coin purse, 6 = "INN",
# 12 = ring, ...), tile 15 is the same board BLANK. The town map has the
# blank board painted on layer 3 directly on top of authored icon signs on
# layer 2 at four cells (the inn/shop doors + one doorless building), and
# since the engine draws layer 3 over layer 2 (rmmz_core.js _addSpot,
# lines 2436-2463 - no plugin in this game overrides it), even the
# original RPGMaker build showed blank boards there. That contradicts the
# obvious authoring intent (why place an INN sign above the inn door just
# to cover it?), so we treat it as an editor-stacking mishap and repair it
# at conversion time: drop the blank board wherever it buries a real sign.
# A lone blank board (the Team Burnout door) is authored content and stays.
OUTSIDE_B_SHEET_NAME = "Outside_B_VS"
BLANK_SIGN_TILE_ID = 15
ICON_SIGN_TILE_IDS = range(1, 15)


def repair_buried_signs(rpg_data, tileset_entry):
    """Clear layer-3 blank sign boards that occlude an authored icon sign
    on layer 2. Mutates rpg_data['data'] in place; returns the number of
    cells repaired. Only meaningful (and only applied) when this tileset's
    B sheet is the VisuStella Outside one - the same tile IDs are entirely
    different art in other B sheets."""
    if tileset_entry['tilesetNames'][SET_NUMBER_B] != OUTSIDE_B_SHEET_NAME:
        return 0

    width = rpg_data['width']
    height = rpg_data['height']
    data = rpg_data['data']
    repaired = 0
    for y in range(height):
        for x in range(width):
            index2 = (2 * height + y) * width + x
            index3 = (3 * height + y) * width + x
            if data[index3] == BLANK_SIGN_TILE_ID and data[index2] in ICON_SIGN_TILE_IDS:
                data[index3] = 0
                repaired += 1
    return repaired


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
    passability = [0] * num_cells
    counters = []

    # Direction bits, identical values to RPGMaker's flag nibble and to
    # tilemap.rs's PASS_* constants: 1=down, 2=left, 4=right, 8=up.
    direction_bits = (0x01, 0x02, 0x04, 0x08)

    def is_wall_top(tile_id):
        """A4 'ceiling' autotiles - the roof band of interior partition
        walls. VisuStella flags them impassable only from above (0xe08/
        0xe0a), which the real engine honors too: wherever a partition has
        a doorway gap, the player can legally sidestep from the gap INTO
        the wall top and stroll along inside the wall. Engine-faithful,
        design-nonsense. This game has no behind-wall gameplay, so wall
        tops are never walkable, period. A4 kinds are 80-127 in rows of 8
        alternating ceiling/face; the classification is confirmed by the
        flags themselves (ceiling kinds carry directional nibbles, face
        kinds are 0x0F solid)."""
        if not (TILE_ID_A4 <= tile_id < 8192):
            return False
        kind = (tile_id - TILE_ID_A1) // 48
        return ((kind - 80) // 8) % 2 == 0

    def check_passage(x, y, bit):
        """Game_Map.checkPassage (rmmz_objects.js): scan this cell's tile
        layers TOP-DOWN; the first layer that isn't a star (0x10) tile
        decides - passable if the direction bit is clear, impassable if
        set. This replaces an earlier any-layer-fully-blocked heuristic
        (plus a forced-solid border ring) that couldn't represent
        directional tiles at all: shop counters, storefront edges, and
        interior wall bands are all "passable from some sides only",
        which the heuristic silently treated as fully walkable."""
        for z in (3, 2, 1, 0):
            flag = flags[get_map_plane(data, width, height, z, x, y)]
            if flag & 0x10:
                continue
            if (flag & bit) == 0:
                return True
            if (flag & bit) == bit:
                return False
        return False

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

            mask = 0
            for bit in direction_bits:
                if check_passage(x, y, bit):
                    mask |= bit
            if any(is_wall_top(t) for t in (tile_id0, tile_id1, tile_id2, tile_id3)):
                mask = 0
            passability[index] = mask
            # RPGMaker's Counter flag (0x80, Game_Map.isCounter): the action
            # button reaches ONE tile across a counter-flagged tile, which is
            # how shopkeepers behind counters are talkable. Sparse [x, y]
            # list - a map has at most a few dozen counter cells.
            if any(
                flags[t] & 0x80
                for t in (tile_id0, tile_id1, tile_id2, tile_id3)
                if t
            ):
                counters.append([x, y])
            # Back-compat projection for map JSON consumers that predate
            # passability: fully blocked = not passable in any direction.
            # No border hack needed anymore: border walls block via their
            # directional flags exactly as the real engine intends, and
            # out-of-map moves fail in the runtime regardless.
            collision[index] = mask == 0

    stats = {
        "blocked_cells": sum(collision),
        "upper_cells": sum(1 for v in upper_tiles if v != 0),
    }
    return tiles, upper_tiles, collision, passability, counters, stats


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

    repaired_signs = repair_buried_signs(rpg_data, tileset_entry)
    if repaired_signs:
        print(f"  Repaired {repaired_signs} buried sign(s) in {rpgmaker_map_path.name}")

    width = rpg_data['width']
    height = rpg_data['height']
    tileset_id = rpg_data['tilesetId']

    tiles, upper_tiles, collision, passability, counters, stats = convert_tiles_and_collision(
        rpg_data, tileset_entry, compositor
    )

    npcs = extract_npcs(rpg_data, rpgmaker_map_path.name)
    exits = extract_exits_from_events(rpg_data['events'])
    doors = extract_doors(rpg_data['events'], rpgmaker_map_path.name)
    props = extract_props(rpg_data['events'], npcs, doors)

    clean_data = {
        "name": rpg_data['displayName'],
        "width": width,
        "height": height,
        "tiles": tiles,
        "upper_tiles": upper_tiles,
        "collision": collision,
        "passability": passability,
        "counters": counters,
        "npcs": npcs,
        "exits": exits,
        "doors": doors,
        "props": props,
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
        f"Upper-layer cells: {stats['upper_cells']}  "
        f"Counter cells: {len(counters)}"
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
            ("Map005.json", "team_disco.json"),
            ("Map006.json", "team_inferno.json"),
            ("Map007.json", "mahogany_row.json"),
            ("Map010.json", "intro.json"),
            ("Map003.json", "end.json"),
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
