"""
RPGMaker MZ tile-format decoder and compositor.

This is a faithful Python port of the tile ID classification and autotile
compositing algorithm implemented in RPGMaker MZ's own engine core:

    /home/atobey/src/endgame-of-sre-rpgmaker-mz/js/rmmz_core.js
    (the `Tilemap` prototype, roughly lines 2380-2870)

RPGMaker MZ tile IDs already fully encode which of the (up to) 48 autotile
shape-variants to draw -- the map editor bakes the correct shape into the ID
when a human paints a tile in the editor. There is no runtime neighbor
analysis here, just arithmetic decoding + table lookups, exactly mirroring
the original engine.

This module intentionally only ports the subset of `Tilemap` needed to
render a *static* (non-animated) composited tile image per map cell:
water/waterfall animation is baked at frame 0 (see BAKED_ANIMATION_FRAME).

All RPGMaker-specific complexity is meant to stay in this offline build
tool. The Bevy/Rust side only ever sees plain 0-based indices into a
pre-composited PNG atlas.
"""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image

TILE_SIZE = 48  # confirmed empirically: every *_VS.png sheet's dimensions
                # are exact multiples of 48px (see e.g. Outside_A1_VS.png
                # = 768x576 = 16x12 tiles).

# We only ever bake the first animation frame (see project brief: animated
# water/waterfall tiles are a deliberate, approved scope cut).
BAKED_ANIMATION_FRAME = 0

# ---------------------------------------------------------------------------
# Tile ID range constants (rmmz_core.js lines 2667-2676)
# ---------------------------------------------------------------------------
TILE_ID_B = 0
TILE_ID_C = 256
TILE_ID_D = 512
TILE_ID_E = 768
TILE_ID_A5 = 1536
TILE_ID_A1 = 2048
TILE_ID_A2 = 2816
TILE_ID_A3 = 4352
TILE_ID_A4 = 5888
TILE_ID_MAX = 8192

# Index into the `tilesetNames` array from Tilesets.json (and thus the
# `sheets` dict passed around below) -- this order is exactly the order
# RPGMaker itself uses: [A1, A2, A3, A4, A5, B, C, D, E].
SET_NUMBER_A1 = 0
SET_NUMBER_A2 = 1
SET_NUMBER_A3 = 2
SET_NUMBER_A4 = 3
SET_NUMBER_A5 = 4
SET_NUMBER_B = 5
SET_NUMBER_C = 6
SET_NUMBER_D = 7


# ---------------------------------------------------------------------------
# Tile classification (rmmz_core.js lines 2678-2724, 2750-2752)
# ---------------------------------------------------------------------------
def is_visible_tile(tile_id: int) -> bool:
    return 0 < tile_id < TILE_ID_MAX


def is_autotile(tile_id: int) -> bool:
    return tile_id >= TILE_ID_A1


def get_autotile_kind(tile_id: int) -> int:
    return (tile_id - TILE_ID_A1) // 48


def get_autotile_shape(tile_id: int) -> int:
    return (tile_id - TILE_ID_A1) % 48


def is_tile_a1(tile_id: int) -> bool:
    return TILE_ID_A1 <= tile_id < TILE_ID_A2


def is_tile_a2(tile_id: int) -> bool:
    return TILE_ID_A2 <= tile_id < TILE_ID_A3


def is_tile_a3(tile_id: int) -> bool:
    return TILE_ID_A3 <= tile_id < TILE_ID_A4


def is_tile_a4(tile_id: int) -> bool:
    return TILE_ID_A4 <= tile_id < TILE_ID_MAX


def is_tile_a5(tile_id: int) -> bool:
    return TILE_ID_A5 <= tile_id < TILE_ID_A1


def is_shadowing_tile(tile_id: int) -> bool:
    # Tilemap.isShadowingTile: used to gate the table-edge special case.
    return is_tile_a3(tile_id) or is_tile_a4(tile_id)


# ---------------------------------------------------------------------------
# Per-tile passability/flag bits (rmmz_core.js lines 2638-2644)
#
# `flags` here is `Tilesets.json`'s `tilesets[id]['flags']` -- a flat array
# of exactly 8192 ints, directly indexed by absolute tile ID.
# ---------------------------------------------------------------------------
def is_higher_tile(flags: list[int], tile_id: int) -> bool:
    # Tilemap.prototype._isHigherTile: `this.flags[tileId] & 0x10`
    return bool(flags[tile_id] & 0x10)


def is_table_tile(flags: list[int], tile_id: int) -> bool:
    # Tilemap.prototype._isTableTile:
    # `Tilemap.isTileA2(tileId) && this.flags[tileId] & 0x80`
    return is_tile_a2(tile_id) and bool(flags[tile_id] & 0x80)


def is_fully_blocked(flags: list[int], tile_id: int) -> bool:
    """
    Collision heuristic (empirically confirmed against Tilesets.json +
    Map002.json -- see PLAN/report notes): the low nibble of the flags
    value holds 4 directional-passage-blocked bits (down/left/right/up).
    When all 4 are set (0x0F) the tile is impassable from every direction,
    i.e. "fully blocked". Partial/directional passability is out of scope.
    """
    return tile_id > 0 and (flags[tile_id] & 0x0F) == 0x0F


# ---------------------------------------------------------------------------
# Autotile shape -> source-quadrant lookup tables
# (rmmz_core.js lines 2793-2870, extracted verbatim via a JSON-parse of the
# literal JS array source so there is zero risk of manual transcription
# error -- see tools/README or git history for the extraction one-liner.)
# Each entry is 4 [qsx, qsy] pairs, one per destination quadrant (top-left,
# top-right, bottom-left, bottom-right).
# ---------------------------------------------------------------------------
FLOOR_AUTOTILE_TABLE = [[[2, 4], [1, 4], [2, 3], [1, 3]], [[2, 0], [1, 4], [2, 3], [1, 3]], [[2, 4], [3, 0], [2, 3], [1, 3]], [[2, 0], [3, 0], [2, 3], [1, 3]], [[2, 4], [1, 4], [2, 3], [3, 1]], [[2, 0], [1, 4], [2, 3], [3, 1]], [[2, 4], [3, 0], [2, 3], [3, 1]], [[2, 0], [3, 0], [2, 3], [3, 1]], [[2, 4], [1, 4], [2, 1], [1, 3]], [[2, 0], [1, 4], [2, 1], [1, 3]], [[2, 4], [3, 0], [2, 1], [1, 3]], [[2, 0], [3, 0], [2, 1], [1, 3]], [[2, 4], [1, 4], [2, 1], [3, 1]], [[2, 0], [1, 4], [2, 1], [3, 1]], [[2, 4], [3, 0], [2, 1], [3, 1]], [[2, 0], [3, 0], [2, 1], [3, 1]], [[0, 4], [1, 4], [0, 3], [1, 3]], [[0, 4], [3, 0], [0, 3], [1, 3]], [[0, 4], [1, 4], [0, 3], [3, 1]], [[0, 4], [3, 0], [0, 3], [3, 1]], [[2, 2], [1, 2], [2, 3], [1, 3]], [[2, 2], [1, 2], [2, 3], [3, 1]], [[2, 2], [1, 2], [2, 1], [1, 3]], [[2, 2], [1, 2], [2, 1], [3, 1]], [[2, 4], [3, 4], [2, 3], [3, 3]], [[2, 4], [3, 4], [2, 1], [3, 3]], [[2, 0], [3, 4], [2, 3], [3, 3]], [[2, 0], [3, 4], [2, 1], [3, 3]], [[2, 4], [1, 4], [2, 5], [1, 5]], [[2, 0], [1, 4], [2, 5], [1, 5]], [[2, 4], [3, 0], [2, 5], [1, 5]], [[2, 0], [3, 0], [2, 5], [1, 5]], [[0, 4], [3, 4], [0, 3], [3, 3]], [[2, 2], [1, 2], [2, 5], [1, 5]], [[0, 2], [1, 2], [0, 3], [1, 3]], [[0, 2], [1, 2], [0, 3], [3, 1]], [[2, 2], [3, 2], [2, 3], [3, 3]], [[2, 2], [3, 2], [2, 1], [3, 3]], [[2, 4], [3, 4], [2, 5], [3, 5]], [[2, 0], [3, 4], [2, 5], [3, 5]], [[0, 4], [1, 4], [0, 5], [1, 5]], [[0, 4], [3, 0], [0, 5], [1, 5]], [[0, 2], [3, 2], [0, 3], [3, 3]], [[0, 2], [1, 2], [0, 5], [1, 5]], [[0, 4], [3, 4], [0, 5], [3, 5]], [[2, 2], [3, 2], [2, 5], [3, 5]], [[0, 2], [3, 2], [0, 5], [3, 5]], [[0, 0], [1, 0], [0, 1], [1, 1]]]

WALL_AUTOTILE_TABLE = [[[2, 2], [1, 2], [2, 1], [1, 1]], [[0, 2], [1, 2], [0, 1], [1, 1]], [[2, 0], [1, 0], [2, 1], [1, 1]], [[0, 0], [1, 0], [0, 1], [1, 1]], [[2, 2], [3, 2], [2, 1], [3, 1]], [[0, 2], [3, 2], [0, 1], [3, 1]], [[2, 0], [3, 0], [2, 1], [3, 1]], [[0, 0], [3, 0], [0, 1], [3, 1]], [[2, 2], [1, 2], [2, 3], [1, 3]], [[0, 2], [1, 2], [0, 3], [1, 3]], [[2, 0], [1, 0], [2, 3], [1, 3]], [[0, 0], [1, 0], [0, 3], [1, 3]], [[2, 2], [3, 2], [2, 3], [3, 3]], [[0, 2], [3, 2], [0, 3], [3, 3]], [[2, 0], [3, 0], [2, 3], [3, 3]], [[0, 0], [3, 0], [0, 3], [3, 3]]]

WATERFALL_AUTOTILE_TABLE = [[[2, 0], [1, 0], [2, 1], [1, 1]], [[0, 0], [1, 0], [0, 1], [1, 1]], [[2, 0], [3, 0], [2, 1], [3, 1]], [[0, 0], [3, 0], [0, 1], [3, 1]]]


# ---------------------------------------------------------------------------
# Sheet loading
# ---------------------------------------------------------------------------
def load_tileset_sheets(tileset_names: list[str], tilesets_dir: Path) -> dict[int, Image.Image]:
    """
    `tileset_names` is `Tilesets.json`'s `tilesets[id]['tilesetNames']`:
    a 9-entry array `[A1, A2, A3, A4, A5, B, C, D, E]` (some entries may be
    the empty string when that sheet isn't used by this tileset, e.g. the
    "Inside" tileset has no A3 or D sheet).

    Returns a dict keyed by "set number" (0=A1 .. 7=D, matching the array
    order above) to a loaded RGBA image. Missing/empty sheets are simply
    absent from the dict.
    """
    sheets: dict[int, Image.Image] = {}
    for set_number, name in enumerate(tileset_names):
        if not name:
            continue
        path = Path(tilesets_dir) / f"{name}.png"
        if not path.exists():
            raise FileNotFoundError(f"tileset sheet not found: {path}")
        image = Image.open(path).convert("RGBA")
        if image.width % TILE_SIZE != 0 or image.height % TILE_SIZE != 0:
            raise ValueError(
                f"{path} is {image.width}x{image.height}px, not a multiple "
                f"of TILE_SIZE={TILE_SIZE}px"
            )
        sheets[set_number] = image
    return sheets


class MissingTilesetSheetError(ValueError):
    """Raised when a tile ID decodes to a sheet slot this tileset doesn't
    provide (e.g. an A3/D tile ID painted on a map that uses the "Inside"
    tileset, which has no A3 or D sheet). This is treated as a soft,
    loudly-logged fallback by render_tile() rather than a hard crash,
    because it has been empirically confirmed (see project notes) to
    reflect a handful of genuinely stray/leftover authoring artifacts in
    the original RPGMaker map data, not a bug in this decoder -- unlike an
    out-of-bounds source rect *within* a sheet that does exist, which
    always indicates a real arithmetic bug and must stay a hard failure."""


def _sheet_for(sheets: dict[int, Image.Image], set_number: int, tile_id: int) -> Image.Image:
    sheet = sheets.get(set_number)
    if sheet is None:
        raise MissingTilesetSheetError(
            f"tile {tile_id} needs tileset sheet #{set_number}, but this "
            f"tileset does not provide one (empty tilesetNames slot)"
        )
    return sheet


def _assert_in_bounds(sheet: Image.Image, x: int, y: int, w: int, h: int, tile_id: int, set_number: int) -> None:
    if x < 0 or y < 0 or x + w > sheet.width or y + h > sheet.height:
        raise ValueError(
            f"tile {tile_id} (sheet #{set_number}) wants source rect "
            f"({x},{y},{x + w},{y + h}) but sheet is only "
            f"{sheet.width}x{sheet.height}px -- tile ID decoding is wrong, "
            f"refusing to silently crop/corrupt"
        )


def _crop(sheet: Image.Image, x: int, y: int, w: int, h: int, tile_id: int, set_number: int) -> Image.Image:
    _assert_in_bounds(sheet, x, y, w, h, tile_id, set_number)
    return sheet.crop((x, y, x + w, y + h))


# ---------------------------------------------------------------------------
# Normal (non-autotile) tile source-rect math
# (rmmz_core.js _addNormalTile, lines 2483-2498)
# ---------------------------------------------------------------------------
def render_normal_tile(tile_id: int, sheets: dict[int, Image.Image]) -> Image.Image:
    if is_tile_a5(tile_id):
        set_number = SET_NUMBER_A5
    else:
        set_number = 5 + (tile_id // 256)

    sx = ((tile_id // 128 % 2) * 8 + (tile_id % 8)) * TILE_SIZE
    sy = (tile_id % 256 // 8 % 16) * TILE_SIZE

    sheet = _sheet_for(sheets, set_number, tile_id)
    return _crop(sheet, sx, sy, TILE_SIZE, TILE_SIZE, tile_id, set_number)


# ---------------------------------------------------------------------------
# Autotile source-rect math, 4 quadrants per tile
# (rmmz_core.js _addAutotile, lines 2500-2577)
# ---------------------------------------------------------------------------
def render_autotile(tile_id: int, sheets: dict[int, Image.Image], flags: list[int]) -> Image.Image:
    kind = get_autotile_kind(tile_id)
    shape = get_autotile_shape(tile_id)
    tx = kind % 8
    ty = kind // 8

    set_number = 0
    bx = 0
    by = 0
    autotile_table = FLOOR_AUTOTILE_TABLE
    is_table = False

    if is_tile_a1(tile_id):
        water_surface_index = [0, 1, 2, 1][BAKED_ANIMATION_FRAME % 4]
        set_number = SET_NUMBER_A1
        if kind == 0:
            bx = water_surface_index * 2
            by = 0
        elif kind == 1:
            bx = water_surface_index * 2
            by = 3
        elif kind == 2:
            bx = 6
            by = 0
        elif kind == 3:
            bx = 6
            by = 3
        else:
            bx = (tx // 4) * 8
            by = ty * 6 + (tx // 2 % 2) * 3
            if kind % 2 == 0:
                bx += water_surface_index * 2
            else:
                bx += 6
                autotile_table = WATERFALL_AUTOTILE_TABLE
                by += BAKED_ANIMATION_FRAME % 3
    elif is_tile_a2(tile_id):
        set_number = SET_NUMBER_A2
        bx = tx * 2
        by = (ty - 2) * 3
        is_table = is_table_tile(flags, tile_id)
    elif is_tile_a3(tile_id):
        set_number = SET_NUMBER_A3
        bx = tx * 2
        by = (ty - 6) * 2
        autotile_table = WALL_AUTOTILE_TABLE
    elif is_tile_a4(tile_id):
        set_number = SET_NUMBER_A4
        bx = tx * 2
        by = math.floor((ty - 10) * 2.5 + (0.5 if ty % 2 == 1 else 0))
        if ty % 2 == 1:
            autotile_table = WALL_AUTOTILE_TABLE
    else:
        raise ValueError(f"tile {tile_id} is not an autotile (not A1-A4)")

    table = autotile_table[shape]
    sheet = _sheet_for(sheets, set_number, tile_id)

    w1 = h1 = TILE_SIZE // 2
    out = Image.new("RGBA", (TILE_SIZE, TILE_SIZE), (0, 0, 0, 0))
    for i in range(4):
        qsx, qsy = table[i]
        sx1 = (bx * 2 + qsx) * w1
        sy1 = (by * 2 + qsy) * h1
        dx1 = (i % 2) * w1
        dy1 = (i // 2) * h1

        if is_table and qsy in (1, 5):
            # "Table" (desk/counter) autotiles show both a top surface and
            # a thin front-lip edge in a single tile: draw the plain floor
            # quadrant first, then overwrite its bottom half with the
            # table-front graphic (rmmz_core.js lines 2566-2572).
            qsx2 = (4 - qsx) % 4 if qsy == 1 else qsx
            qsy2 = 3
            sx2 = (bx * 2 + qsx2) * w1
            sy2 = (by * 2 + qsy2) * h1
            full = _crop(sheet, sx2, sy2, w1, h1, tile_id, set_number)
            out.alpha_composite(full, dest=(dx1, dy1))
            half = _crop(sheet, sx1, sy1, w1, h1 // 2, tile_id, set_number)
            out.alpha_composite(half, dest=(dx1, dy1 + h1 // 2))
        else:
            quad = _crop(sheet, sx1, sy1, w1, h1, tile_id, set_number)
            out.alpha_composite(quad, dest=(dx1, dy1))

    return out


_warned_missing_sheets: set[int] = set()
_warned_blank_tiles: set[int] = set()


def render_tile(tile_id: int, sheets: dict[int, Image.Image], flags: list[int]) -> Image.Image | None:
    """Top-level per-tile-ID renderer (rmmz_core.js Tilemap._addTile). Returns
    None for tile ID 0 / out-of-range IDs (nothing to draw), and also (with a
    loud warning) for tile IDs that reference a tileset sheet this tileset
    doesn't provide -- see MissingTilesetSheetError."""
    if not is_visible_tile(tile_id):
        return None
    try:
        if is_autotile(tile_id):
            image = render_autotile(tile_id, sheets, flags)
        else:
            image = render_normal_tile(tile_id, sheets)
    except MissingTilesetSheetError as error:
        if tile_id not in _warned_missing_sheets:
            _warned_missing_sheets.add(tile_id)
            print(f"WARNING: {error} -- rendering as invisible")
        return None

    # A visible tile ID whose source rect(s) land entirely within a real,
    # present sheet but decode to 100% transparent pixels is a different
    # failure mode than MissingTilesetSheetError: the sheet exists and the
    # coordinate math is in-bounds, but there is simply no authored artwork
    # at that exact position (e.g. the map was painted against a richer
    # tileset before an asset swap left gaps in the replacement sheet -- see
    # e.g. Inside_A5_VS.png row 6, which is blank except column 6). Silently
    # compositing nothing here is indistinguishable from a genuine "blank
    # decoration" tile, which is exactly the kind of silent fallback that
    # hides real bugs, so it's surfaced with the same loud-once treatment as
    # a missing sheet rather than passed through quietly. Nothing about the
    # rendered pixels changes -- there's no valid source art to substitute --
    # this only makes the gap visible in build output instead of invisible.
    if tile_id not in _warned_blank_tiles and image.getextrema()[3][1] == 0:
        _warned_blank_tiles.add(tile_id)
        print(
            f"WARNING: tile {tile_id} decoded in-bounds but is 100% "
            f"transparent (sheet has no art at that position) -- "
            f"rendering as invisible"
        )
    return image


# ---------------------------------------------------------------------------
# Shadow pen (rmmz_core.js _addShadow, lines 2604-2616; shadow color
# confirmed as flat black at 50% opacity from the tilemap fragment shader,
# rmmz_core.js line 3117: `color = vec4(0.0, 0.0, 0.0, 0.5);`)
# ---------------------------------------------------------------------------
def apply_shadow(canvas: Image.Image, shadow_bits: int) -> None:
    if not (shadow_bits & 0x0F):
        return
    w1 = h1 = TILE_SIZE // 2
    shadow_quad = Image.new("RGBA", (w1, h1), (0, 0, 0, 128))
    for i in range(4):
        if shadow_bits & (1 << i):
            dx1 = (i % 2) * w1
            dy1 = (i // 2) * h1
            canvas.alpha_composite(shadow_quad, dest=(dx1, dy1))


# ---------------------------------------------------------------------------
# Table-edge special case (rmmz_core.js _addTableEdge, lines 2579-2602):
# when the cell directly above (in tile-data layer 1) is a "table" (desk /
# counter) autotile and *this* cell's layer-1 tile is not, draw a thin strip
# across the top of *this* cell so the table's front face visually
# continues downward. Only applies to A2 autotiles.
# ---------------------------------------------------------------------------
def render_table_edge_overlay(upper_tile_id1: int, sheets: dict[int, Image.Image]) -> Image.Image | None:
    if not is_tile_a2(upper_tile_id1):
        return None

    kind = get_autotile_kind(upper_tile_id1)
    shape = get_autotile_shape(upper_tile_id1)
    tx = kind % 8
    ty = kind // 8
    set_number = SET_NUMBER_A2
    bx = tx * 2
    by = (ty - 2) * 3
    table = FLOOR_AUTOTILE_TABLE[shape]
    sheet = _sheet_for(sheets, set_number, upper_tile_id1)

    w1 = h1 = TILE_SIZE // 2
    out = Image.new("RGBA", (TILE_SIZE, TILE_SIZE), (0, 0, 0, 0))
    for i in range(2):
        qsx, qsy = table[2 + i]
        sx1 = (bx * 2 + qsx) * w1
        sy1 = (by * 2 + qsy) * h1 + h1 // 2
        dx1 = (i % 2) * w1
        dy1 = (i // 2) * h1  # always 0: this strip sits at the top of the cell
        strip = _crop(sheet, sx1, sy1, w1, h1 // 2, upper_tile_id1, set_number)
        out.alpha_composite(strip, dest=(dx1, dy1))
    return out
