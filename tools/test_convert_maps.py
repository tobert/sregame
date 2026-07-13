#!/usr/bin/env python3
"""
Plain-assert regression pins for convert_maps.py's data-repair passes.

Same convention as test_rpgmaker_tiles.py: no test framework, a standalone
script (`python3 tools/test_convert_maps.py`). Unlike that file, these
tests DO read the real RPGMaker project data (RPGMAKER_DATA_DIR), because
what they pin is the interaction between a repair pass and the actual
shipped map/tileset content.
"""

from __future__ import annotations

import copy
import json
import sys

import convert_maps as cm

FAILURES: list[str] = []


def check(label: str, actual, expected) -> None:
    if actual != expected:
        FAILURES.append(f"{label}: expected {expected!r}, got {actual!r}")


def load_rpg_map(name: str) -> dict:
    with open(cm.RPGMAKER_DATA_DIR / name, encoding="utf-8") as f:
        return json.load(f)


def load_tileset_for(rpg_data: dict) -> dict:
    return cm.load_tilesets_json()[rpg_data["tilesetId"]]


def plane(rpg_data: dict, z: int, x: int, y: int) -> int:
    w, h = rpg_data["width"], rpg_data["height"]
    return rpg_data["data"][(z * h + y) * w + x]


# The four town cells where the original author's icon/text signs are
# buried under a blank sign board (tile 15) painted on layer 3. Found by
# scanning every map for B-sheet-over-B-sheet layer stacks; these are the
# only blank-over-sign cases in the whole game.
TOWN_BURIED_SIGNS = {
    (8, 27): 6,    # "INN" sign, above the To Inn door
    (23, 18): 4,   # coin-purse sign, above the To Team Disconnect door
    (29, 11): 12,  # ring sign, above the To Product Team door
    (16, 4): 1,    # sword sign, on the doorless fifth building up north
}


def test_town_buried_signs_are_unburied() -> None:
    rpg = load_rpg_map("Map002.json")
    repaired = cm.repair_buried_signs(rpg, load_tileset_for(rpg))
    check("repaired count", repaired, len(TOWN_BURIED_SIGNS))
    for (x, y), sign_tile in TOWN_BURIED_SIGNS.items():
        check(f"blank sign removed at ({x},{y})", plane(rpg, 3, x, y), 0)
        check(f"authored sign kept at ({x},{y})", plane(rpg, 2, x, y), sign_tile)


def test_lone_blank_sign_is_kept() -> None:
    # The Team Burnout door (6,16) has ONLY the blank sign - nothing is
    # buried under it, so removing it would delete authored content.
    rpg = load_rpg_map("Map002.json")
    cm.repair_buried_signs(rpg, load_tileset_for(rpg))
    check("lone blank sign layer 3", plane(rpg, 3, 6, 16), 15)
    check("lone blank sign layer 2", plane(rpg, 2, 6, 16), 0)


def test_repair_only_touches_the_buried_sign_cells() -> None:
    rpg = load_rpg_map("Map002.json")
    before = copy.deepcopy(rpg["data"])
    cm.repair_buried_signs(rpg, load_tileset_for(rpg))
    w, h = rpg["width"], rpg["height"]
    changed = {
        (i % w, (i // w) % h)
        for i, (a, b) in enumerate(zip(before, rpg["data"]))
        if a != b
    }
    check("changed cells", changed, set(TOWN_BURIED_SIGNS))


def test_inside_tileset_maps_are_untouched() -> None:
    # Tile IDs 1-15 mean completely different art in the Inside tileset's
    # B sheet; the repair is only valid against Outside_B_VS. Map004 even
    # has a (layer2=9, layer3=11) stack at (2,9) that superficially looks
    # like the pattern - it must survive.
    for name in ("Map004.json", "Map005.json", "Map009.json"):
        rpg = load_rpg_map(name)
        before = list(rpg["data"])
        check(f"{name} repaired count", cm.repair_buried_signs(rpg, load_tileset_for(rpg)), 0)
        check(f"{name} data unchanged", rpg["data"] == before, True)


def fresh_end_clean_data() -> dict:
    # Minimal clean_data shape for the End map (17x13): all-impassable
    # passability, no exits/props - what convert_map assembles before the
    # portal pass runs.
    return {
        "width": 17,
        "height": 13,
        "passability": [0] * (17 * 13),
        "exits": [],
        "props": [],
    }


def test_end_gets_return_portals() -> None:
    data = fresh_end_clean_data()
    check("applied to end.json", cm.add_end_return_portals(data, "end.json"), True)

    check("exit count", len(data["exits"]), 2)
    for exit_data, (x, y) in zip(data["exits"], cm.END_PORTAL_TILES):
        check(f"exit at ({x},{y}) trigger_x", exit_data["trigger_x"], x)
        check(f"exit at ({x},{y}) trigger_y", exit_data["trigger_y"], y)
        check(f"exit at ({x},{y}) target", exit_data["target_scene"], "TownOfEndgame")
        check(f"exit at ({x},{y}) trigger type", exit_data["trigger"], "touch")

    check("prop count", len(data["props"]), 2)
    for prop, (x, y) in zip(data["props"], cm.END_PORTAL_TILES):
        check(f"fairy at ({x},{y})", (prop["x"], prop["y"]), (x, y))
        check(f"fairy at ({x},{y}) flutters", prop["step_anime"], True)
        check(f"fairy at ({x},{y}) doesn't block", prop["blocks"], False)

    # Player pocket opened, everything else still sealed.
    w = data["width"]
    opened = {(i % w, i // w) for i, n in enumerate(data["passability"]) if n != 0}
    check("opened cells", opened, set(cm.END_PLAYER_POCKET))


def test_return_portals_only_apply_to_end() -> None:
    data = fresh_end_clean_data()
    check("not applied elsewhere", cm.add_end_return_portals(data, "town_of_endgame.json"), False)
    check("no exits added", data["exits"], [])
    check("no props added", data["props"], [])
    check("passability untouched", any(data["passability"]), False)


def main() -> int:
    test_town_buried_signs_are_unburied()
    test_lone_blank_sign_is_kept()
    test_repair_only_touches_the_buried_sign_cells()
    test_inside_tileset_maps_are_untouched()
    test_end_gets_return_portals()
    test_return_portals_only_apply_to_end()

    if FAILURES:
        print(f"FAILED ({len(FAILURES)}):")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1

    print("All convert_maps.py repair-pass regression pins passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
