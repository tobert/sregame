use bevy::prelude::*;
use crate::game_state::GameState;
use crate::player::Player;
use crate::npc::Npc;
use crate::camera::MainCamera;

pub struct SemanticViewportPlugin;

impl Plugin for SemanticViewportPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ViewportUpdateTimer(Timer::from_seconds(1.0, TimerMode::Repeating)))
            .add_systems(Update, log_viewport_state.run_if(in_state(GameState::Playing)));
    }
}

#[derive(Resource)]
struct ViewportUpdateTimer(Timer);

fn log_viewport_state(
    time: Res<Time>,
    mut timer: ResMut<ViewportUpdateTimer>,
    camera_query: Query<&GlobalTransform, With<MainCamera>>,
    player_query: Query<(Entity, &GlobalTransform), With<Player>>,
    npc_query: Query<(Entity, &GlobalTransform, &Npc), With<Npc>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let Some(camera_transform) = camera_query.iter().next() else {
        return;
    };

    // Viewport logic
    // Base resolution: 960x540
    // Tile size: 48x48
    // Grid dimensions: 20x12 (covers 960x576)
    const TILE_SIZE: f32 = 48.0;
    const GRID_W: usize = 20;
    const GRID_H: usize = 12;
    
    let cam_pos = camera_transform.translation().truncate();
    
    // Calculate top-left corner of the visible grid in world space
    // Bevy is Y-up, so Top is Y + Height/2
    // We align to tile boundaries for stability
    let view_width = GRID_W as f32 * TILE_SIZE;
    let view_height = GRID_H as f32 * TILE_SIZE;
    
    let min_x = cam_pos.x - view_width / 2.0;
    let max_y = cam_pos.y + view_height / 2.0;

    // Initialize grid with empty space
    let mut grid = [['.'; GRID_W]; GRID_H];
    let mut legend = Vec::new();

    // Helper to project world pos to grid coords
    let world_to_grid = |pos: Vec2| -> Option<(usize, usize)> {
        let rel_x = pos.x - min_x;
        let rel_y = max_y - pos.y; // Invert Y for grid (Row 0 is Top)

        if rel_x >= 0.0 && rel_x < view_width && rel_y >= 0.0 && rel_y < view_height {
            let x = (rel_x / TILE_SIZE) as usize;
            let y = (rel_y / TILE_SIZE) as usize;
            Some((x, y))
        } else {
            None
        }
    };

    // Render Player
    if let Some((_, player_tf)) = player_query.iter().next() {
        let pos = player_tf.translation().truncate();
        if let Some((x, y)) = world_to_grid(pos) {
            grid[y][x] = 'P';
            legend.push(format!("P = Player ({:.0}, {:.0})", pos.x, pos.y));
        }
    }

    // Render NPCs
    for (_, npc_tf, npc) in &npc_query {
        let pos = npc_tf.translation().truncate();
        if let Some((x, y)) = world_to_grid(pos) {
            // If cell is occupied (e.g. by player), use 'X' or priority
            let char = if grid[y][x] == '.' { 'N' } else { '&' };
            grid[y][x] = char;
            legend.push(format!("N = NPC:{} ({:.0}, {:.0})", npc.name, pos.x, pos.y));
        }
    }

    // Format output
    let mut output = String::with_capacity(512);
    output.push_str("\n[VIEWPORT] Semantic Map (20x12)\n");
    
    // Top border
    output.push_str(&"+-".repeat(GRID_W));
    output.push_str("+ \n");

    for row in grid {
        output.push('|');
        for cell in row {
            output.push(cell);
            output.push(' '); // Spacing for readability
        }
        output.push_str("|\n");
    }
    
    // Bottom border
    output.push_str(&"+-".repeat(GRID_W));
    output.push_str("+ \n");

    output.push_str("[LEGEND]\n");
    for item in legend {
        output.push_str(&format!("  {}\n", item));
    }

    info!("{}", output);
}
