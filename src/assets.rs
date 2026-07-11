use bevy::prelude::*;
use crate::game_state::GameState;
use std::collections::HashMap;
use std::fs;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameAssets>()
            .add_systems(OnEnter(GameState::Loading), (
                spawn_loading_screen,
                start_asset_loading,
            ))
            .add_systems(Update, check_asset_loading.run_if(in_state(GameState::Loading)))
            .add_systems(OnExit(GameState::Loading), despawn_loading_screen);
    }
}

#[derive(Resource, Default)]
pub struct GameAssets {
    pub player_sprite: Handle<Image>,
    /// Character sprite sheets, keyed by filename stem (e.g. "Nature" for
    /// `textures/characters/Nature.png`). Populated by scanning the directory
    /// on disk so new characters need no Rust changes.
    pub npc_sprites: HashMap<String, Handle<Image>>,
    /// Tileset textures, keyed by filename stem (e.g. "town_tileset" for
    /// `textures/tilesets/town_tileset.png`). Scenes look these up by the
    /// `tileset_key` in their `SceneConfig` (see `tilemap.rs`).
    pub tilesets: HashMap<String, Handle<Image>>,
    pub portrait_nature: Handle<Image>,
    pub dialogue_font: Handle<Font>,
    pub loaded: bool,
}

#[derive(Component)]
struct LoadingScreen;

fn spawn_loading_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("Spawning loading screen");

    commands.spawn((
        LoadingScreen,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
    ))
    .with_children(|parent| {
        parent.spawn((
            Text::new("The Endgame of SRE"),
            TextFont {
                font: asset_server.load("fonts/dialogue.ttf"),
                font_size: 48.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));

        parent.spawn((
            Text::new("Loading..."),
            TextFont {
                font: asset_server.load("fonts/dialogue.ttf"),
                font_size: 24.0,
                ..default()
            },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
        ));
    });
}

/// Scan a directory on disk for `.png` files and load each one through the
/// asset server, keyed by filename stem. Mirrors the direct-filesystem idiom
/// `map_data.rs` uses for reading map JSON, rather than requiring every asset
/// to be registered by hand.
fn scan_and_load_pngs(
    disk_dir: &str,
    asset_dir: &str,
    asset_server: &AssetServer,
) -> HashMap<String, Handle<Image>> {
    let mut handles = HashMap::new();

    let entries = match fs::read_dir(disk_dir) {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read asset directory {}: {}", disk_dir, e);
            return handles;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("png") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let asset_path = format!("{asset_dir}/{stem}.png");
        handles.insert(stem.to_string(), asset_server.load(asset_path));
    }

    handles
}

fn start_asset_loading(
    mut game_assets: ResMut<GameAssets>,
    asset_server: Res<AssetServer>,
) {
    info!("Starting asset loading...");

    game_assets.player_sprite = asset_server.load("textures/characters/Amy-Walking.png");

    game_assets.npc_sprites = scan_and_load_pngs(
        "assets/textures/characters",
        "textures/characters",
        &asset_server,
    );
    game_assets.tilesets = scan_and_load_pngs(
        "assets/textures/tilesets",
        "textures/tilesets",
        &asset_server,
    );

    info!(
        "Discovered {} character sprites, {} tilesets",
        game_assets.npc_sprites.len(),
        game_assets.tilesets.len()
    );

    game_assets.portrait_nature = asset_server.load("textures/portraits/Nature.png");
    game_assets.dialogue_font = asset_server.load("fonts/dialogue.ttf");

    game_assets.loaded = false;
}

fn check_asset_loading(
    mut game_assets: ResMut<GameAssets>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if game_assets.loaded {
        return;
    }

    let all_loaded = asset_server.is_loaded_with_dependencies(&game_assets.player_sprite)
        && asset_server.is_loaded_with_dependencies(&game_assets.portrait_nature)
        && asset_server.is_loaded_with_dependencies(&game_assets.dialogue_font)
        && game_assets
            .npc_sprites
            .values()
            .all(|handle| asset_server.is_loaded_with_dependencies(handle))
        && game_assets
            .tilesets
            .values()
            .all(|handle| asset_server.is_loaded_with_dependencies(handle));

    if all_loaded {
        game_assets.loaded = true;
        info!("All assets loaded successfully!");
        next_state.set(GameState::Playing);
    }
}

fn despawn_loading_screen(
    mut commands: Commands,
    loading_screen: Query<Entity, With<LoadingScreen>>,
) {
    for entity in &loading_screen {
        commands.entity(entity).despawn();
    }
    info!("Loading screen despawned");
}
