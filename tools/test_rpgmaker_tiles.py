#!/usr/bin/env python3
"""
Plain-assert regression pins for rpgmaker_tiles.py's tile ID decode math.

No test framework is wired up for the Python build tools in this repo (no
pytest dependency, no tools/ test convention) -- this is deliberately a
standalone script (`python3 tools/test_rpgmaker_tiles.py`), not a pytest
suite, so it stays runnable with only the stdlib + the same environment
convert_maps.py already needs (Pillow).

Every expected (set_number, sx, sy) / quadrant tuple below was hand-derived
directly from rmmz_core.js's Tilemap._addNormalTile / _addAutotile source
(see rpgmaker_tiles.py's own module docstring for the exact file), not
copied from this module's own output -- so a future refactor that silently
changes the arithmetic (e.g. an off-by-one in the A4 "ty - 10" row formula,
or a swapped bx/by) will fail loudly here even if it happens to keep
producing *some* in-bounds image. This is exactly the kind of regression
these tile IDs already caught once during the fix-tiles investigation (see
the WARNING-worthy fully-transparent decodes for tile IDs 359, 1544, 1587 --
those are real *data* gaps in the shipped VisuStella art, not decoder bugs,
confirmed by cross-checking this same arithmetic against a from-scratch
reimplementation; they are intentionally not pinned here as "correct
pixels", only the in-bounds source-rect math for other tile IDs is).
"""

from __future__ import annotations

import sys

import rpgmaker_tiles as rt

FAILURES: list[str] = []


def check(label: str, actual, expected) -> None:
    if actual != expected:
        FAILURES.append(f"{label}: expected {expected!r}, got {actual!r}")


def normal_tile_source_rect(tile_id: int) -> tuple[int, int, int]:
    """Recomputes _addNormalTile's (setNumber, sx, sy) without going through
    render_normal_tile's Image cropping, so these checks work without any
    tileset PNGs on disk."""
    if rt.is_tile_a5(tile_id):
        set_number = rt.SET_NUMBER_A5
    else:
        set_number = 5 + (tile_id // 256)
    sx = ((tile_id // 128 % 2) * 8 + (tile_id % 8)) * rt.TILE_SIZE
    sy = (tile_id % 256 // 8 % 16) * rt.TILE_SIZE
    return set_number, sx, sy


def test_normal_tile_b_sheet() -> None:
    # tile 9: rmmz_core.js _addNormalTile with tileId=9 (< TILE_ID_C=256, so
    # "B" sheet, setNumber = 5 + floor(9/256) = 5).
    # sx = ((floor(9/128) % 2) * 8 + 9 % 8) * 48 = ((0 % 2) * 8 + 1) * 48 = 48
    # sy = (floor((9 % 256) / 8) % 16) * 48 = (floor(9/8) % 16) * 48 = 48
    check("tile 9 (B sheet)", normal_tile_source_rect(9), (5, 48, 48))


def test_normal_tile_c_sheet() -> None:
    # tile 359: 256 <= 359 < 512, so "C" sheet, setNumber = 5 + floor(359/256) = 6.
    # sx = ((floor(359/128) % 2) * 8 + 359 % 8) * 48 = ((2 % 2) * 8 + 7) * 48 = 336
    # sy = (floor((359 % 256) / 8) % 16) * 48 = (floor(103/8) % 16) * 48 = 576
    check("tile 359 (C sheet)", normal_tile_source_rect(359), (6, 336, 576))


def test_normal_tile_a5_sheet() -> None:
    # tile 1587: TILE_ID_A5=1536 <= 1587 < TILE_ID_A1=2048, so setNumber = 4
    # regardless of the tileId/256 term (A5's own branch bypasses it).
    # sx = ((floor(1587/128) % 2) * 8 + 1587 % 8) * 48 = ((12 % 2) * 8 + 3) * 48 = 144
    # sy = (floor((1587 % 256) / 8) % 16) * 48 = (floor(51/8) % 16) * 48 = 288
    check("tile 1587 (A5 sheet)", normal_tile_source_rect(1587), (4, 144, 288))


def test_autotile_a2_floor() -> None:
    # tile 2859: TILE_ID_A2=2816 <= 2859 < TILE_ID_A3=4352, so setNumber=1
    # (A2). kind = floor((2859 - 2048) / 48) = 16, shape = (2859-2048) % 48 = 43.
    # tx = 16 % 8 = 0, ty = floor(16/8) = 2. bx = tx*2 = 0, by = (ty-2)*3 = 0.
    # FLOOR_AUTOTILE_TABLE[43] = [[0,2],[3,2],[0,5],[3,5]] (rmmz_core.js's
    # verbatim floor autotile table, 0-indexed).
    kind = rt.get_autotile_kind(2859)
    shape = rt.get_autotile_shape(2859)
    check("tile 2859 kind", kind, 16)
    check("tile 2859 shape", shape, 43)
    tx, ty = kind % 8, kind // 8
    bx, by = tx * 2, (ty - 2) * 3
    check("tile 2859 bx,by", (bx, by), (0, 0))
    table = rt.FLOOR_AUTOTILE_TABLE[shape]
    w1 = h1 = rt.TILE_SIZE // 2
    quads = [((bx * 2 + qsx) * w1, (by * 2 + qsy) * h1) for qsx, qsy in table]
    check("tile 2859 quads", quads, [(0, 48), (24, 48), (0, 120), (24, 120)])


def test_autotile_a4_wall() -> None:
    # tile 7810: TILE_ID_A4=5888 <= 7810 < TILE_ID_MAX=8192, so setNumber=3
    # (A4). kind = floor((7810-2048)/48) = 120, shape = (7810-2048) % 48 = 2.
    # tx = 120 % 8 = 0, ty = floor(120/8) = 15 (odd -> WALL_AUTOTILE_TABLE,
    # and by uses the "+0.5" branch).
    # bx = 0*2 = 0, by = floor((15-10)*2.5 + 0.5) = floor(13.0) = 13.
    kind = rt.get_autotile_kind(7810)
    shape = rt.get_autotile_shape(7810)
    check("tile 7810 kind", kind, 120)
    check("tile 7810 shape", shape, 2)
    tx, ty = kind % 8, kind // 8
    check("tile 7810 tx,ty", (tx, ty), (0, 15))
    import math

    bx = tx * 2
    by = math.floor((ty - 10) * 2.5 + (0.5 if ty % 2 == 1 else 0))
    check("tile 7810 bx,by", (bx, by), (0, 13))
    table = rt.WALL_AUTOTILE_TABLE[shape]
    w1 = h1 = rt.TILE_SIZE // 2
    quads = [((bx * 2 + qsx) * w1, (by * 2 + qsy) * h1) for qsx, qsy in table]
    check(
        "tile 7810 quads",
        quads,
        [(48, 624), (24, 624), (48, 648), (24, 648)],
    )


def test_known_transparent_gaps_are_still_in_bounds() -> None:
    # Regression pin for the exact 3 tile IDs this investigation confirmed
    # decode in-bounds (real sheet, valid source rect) but land on 100%
    # transparent pixels in the shipped Inside/VisuStella art -- i.e.
    # genuine upstream asset gaps, not decoder bugs. If a future change to
    # the shipped tileset PNGs fills in these cells, this assertion will
    # start failing loudly, which is the point: it means the
    # render_tile()-level WARNING for these tile IDs is stale and the gap
    # has been fixed upstream, so the warning (and this pin) should be
    # revisited/removed.
    for tile_id in (359, 1544, 1587):
        check(f"tile {tile_id} is_visible_tile", rt.is_visible_tile(tile_id), True)


def main() -> int:
    test_normal_tile_b_sheet()
    test_normal_tile_c_sheet()
    test_normal_tile_a5_sheet()
    test_autotile_a2_floor()
    test_autotile_a4_wall()
    test_known_transparent_gaps_are_still_in_bounds()

    if FAILURES:
        print(f"FAILED ({len(FAILURES)}):")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1

    print("All rpgmaker_tiles.py decode-math regression pins passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
