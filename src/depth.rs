use bevy::prelude::*;
use bevy::transform::TransformSystems;

/// Y-sorted render depth for characters and props.
///
/// Player, NPCs, and props used to share a flat z=1.0, so whenever two
/// sprites overlapped (which pixel movement allows - the collision box is
/// smaller than the sprite art), their draw order was arbitrary. With
/// y-sorting, whoever's feet are lower on screen draws in front, so an
/// overlap reads as depth: the player standing just south of an NPC renders
/// in front of them, standing north renders behind.
pub struct DepthPlugin;

impl Plugin for DepthPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<YSorted>()
            // PostUpdate, before transform propagation: gameplay systems in
            // Update have settled this frame's positions, and the z we
            // derive still makes it into this frame's GlobalTransform.
            .add_systems(PostUpdate, y_sort.before(TransformSystems::Propagate));
    }
}

/// Everything y-sorted lives in a band around this z, between the door
/// sprites (0.9, always behind characters) and the upper tile layer (2.0).
const CHARACTER_Z_BASE: f32 = 1.0;

/// Chosen so the largest map (town, 39 tiles tall = ±936 world y plus a
/// tall prop's overhang) maps into z = 1.0 ± ~0.025 - comfortably inside
/// the (0.9, 2.0) band above.
const Y_SORT_SCALE: f32 = 1.0 / 40_000.0;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct YSorted {
    /// World-space y of this sprite's *feet* relative to its translation:
    /// -24.0 for a standard 48px character frame centered on its tile. Tall
    /// props whose translation is lifted off the tile center (see the
    /// spawn's y_offset) carry -frame_height/2 instead, so a 48x96 truck
    /// sorts by where it meets the ground, not by its roof.
    pub foot_offset: f32,
}

fn y_sort(mut query: Query<(&mut Transform, &YSorted)>) {
    for (mut transform, sorted) in &mut query {
        let feet_y = transform.translation.y + sorted.foot_offset;
        transform.translation.z = CHARACTER_Z_BASE - feet_y * Y_SORT_SCALE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z_for(translation_y: f32, foot_offset: f32) -> f32 {
        CHARACTER_Z_BASE - (translation_y + foot_offset) * Y_SORT_SCALE
    }

    #[test]
    fn lower_feet_draw_in_front() {
        // Player just south of an NPC (smaller world y) must get the larger
        // z, i.e. render in front.
        let npc_z = z_for(0.0, -24.0);
        let player_z = z_for(-16.0, -24.0);
        assert!(player_z > npc_z, "southern sprite must draw in front");
    }

    #[test]
    fn tall_prop_sorts_by_its_feet_not_its_center() {
        // A 48x96 prop is spawned with its translation lifted 24px above
        // the tile center (tilemap.rs y_offset), foot_offset -48. A
        // character standing on the tile just south of it must draw in
        // front; one tile north must draw behind - both compare against the
        // prop's ground line, not its lifted center.
        let prop_z = z_for(24.0, -48.0); // feet at y = -24
        let south_char_z = z_for(-48.0, -24.0); // feet at y = -72
        let north_char_z = z_for(48.0, -24.0); // feet at y = 24
        assert!(south_char_z > prop_z);
        assert!(north_char_z < prop_z);
    }

    #[test]
    fn z_band_stays_between_doors_and_upper_tiles() {
        // Doors sit at 0.9 and the upper tile layer at 2.0 (tilemap.rs);
        // the extreme feet positions of the tallest map must not escape
        // that band or characters would pop under doors / over roofs.
        for feet_y in [-1000.0_f32, 1000.0] {
            let z = z_for(feet_y, 0.0);
            assert!(z > 0.9 && z < 2.0, "z {z} escaped the character band");
        }
    }
}
