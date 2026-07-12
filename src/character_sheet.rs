use bevy::prelude::*;

/// Geometry of a standard RPGMaker MZ character sheet as shipped in
/// assets/textures/characters/*.png: 576x384 px holding a 4x2 grid of
/// characters ("slots" 0-7), each slot a 3-column (animation pattern) x
/// 4-row (facing: down, left, right, up) block of 48x48 frames.
///
/// Every character texture in this game is a full sheet of this shape (the
/// custom single-character art was exported padded into full sheets too), so
/// this module is the single place that knows how to slice one. An earlier
/// version built a bare 3x4 atlas over the whole texture, which silently
/// rendered slot 0 for every NPC regardless of which character the map data
/// asked for.
pub const FRAME_SIZE: u32 = 48;
pub const SHEET_COLUMNS: u32 = 12;
pub const SHEET_ROWS: u32 = 8;

const SLOT_GRID_COLUMNS: u32 = 4;
const SLOTS_PER_SHEET: u32 = 8;
const PATTERNS_PER_SLOT: u32 = 3;
const FACINGS_PER_SLOT: u32 = 4;

/// The middle animation pattern: the "standing still" frame.
pub const STANDING_PATTERN: u32 = 1;

pub fn sheet_layout() -> TextureAtlasLayout {
    TextureAtlasLayout::from_grid(
        UVec2::splat(FRAME_SIZE),
        SHEET_COLUMNS,
        SHEET_ROWS,
        None,
        None,
    )
}

/// Atlas index of one 48x48 frame within `sheet_layout`. `facing_row` uses
/// RPGMaker row order: 0=down, 1=left, 2=right, 3=up.
///
/// Panics on out-of-range input: sprite data referencing a slot that doesn't
/// exist is corrupt, and quietly rendering some other character's frames
/// would be worse than failing loudly.
pub fn atlas_index(slot: u32, facing_row: u32, pattern: u32) -> u32 {
    assert!(
        slot < SLOTS_PER_SHEET,
        "character slot {slot} out of range (sheets hold {SLOTS_PER_SHEET} characters)"
    );
    assert!(
        facing_row < FACINGS_PER_SLOT,
        "facing row {facing_row} out of range (0=down, 1=left, 2=right, 3=up)"
    );
    assert!(
        pattern < PATTERNS_PER_SLOT,
        "animation pattern {pattern} out of range (slots have {PATTERNS_PER_SLOT} columns)"
    );

    let block_col = slot % SLOT_GRID_COLUMNS;
    let block_row = slot / SLOT_GRID_COLUMNS;
    (block_row * FACINGS_PER_SLOT + facing_row) * SHEET_COLUMNS + block_col * PATTERNS_PER_SLOT + pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_zero_standing_down_is_index_one() {
        // Top-left character, middle column of the top row.
        assert_eq!(atlas_index(0, 0, STANDING_PATTERN), 1);
    }

    #[test]
    fn known_frames_from_real_map_data() {
        // Nanny Ogg Vorbis: People1 slot 7 (bottom-right block), facing down.
        // Block starts at column 3*3=9, row 1*4=4: (4+0)*12 + 9 + 1 = 58.
        assert_eq!(atlas_index(7, 0, STANDING_PATTERN), 58);

        // Vee Peapod (Mahogany Row): People4 slot 6, facing up.
        // Block col 2, row 1: (4+3)*12 + 6 + 1 = 91.
        assert_eq!(atlas_index(6, 3, STANDING_PATTERN), 91);

        // Agi Lecoach (Town): Actor2 slot 3, facing right, mid-walk.
        // Block col 3, row 0: (0+2)*12 + 9 + 2 = 35.
        assert_eq!(atlas_index(3, 2, 2), 35);
    }

    #[test]
    fn last_frame_is_the_last_cell_of_the_sheet() {
        // Slot 7, facing up, last pattern must be the sheet's final cell,
        // proving the index math never escapes the 12x8 grid.
        assert_eq!(
            atlas_index(7, 3, 2),
            SHEET_COLUMNS * SHEET_ROWS - 1
        );
    }

    #[test]
    #[should_panic(expected = "character slot 8 out of range")]
    fn slot_out_of_range_panics() {
        atlas_index(8, 0, 0);
    }
}
