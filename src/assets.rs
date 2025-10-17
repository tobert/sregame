use bevy::prelude::*;
use crate::game_state::GameState;

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

#[derive(Resource)]
pub struct GameAssets {
    pub player_sprite: Handle<Image>,
    pub npc_nature: Handle<Image>,
    pub town_tileset: Handle<Image>,
    pub portrait_nature: Handle<Image>,
    pub dialogue_font: Handle<Font>,
    pub loaded: bool,
}

impl Default for GameAssets {
    fn default() -> Self {
        Self {
            player_sprite: Handle::default(),
            npc_nature: Handle::default(),
            town_tileset: Handle::default(),
            portrait_nature: Handle::default(),
            dialogue_font: Handle::default(),
            loaded: false,
        }
    }
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

fn start_asset_loading(
    mut game_assets: ResMut<GameAssets>,
    asset_server: Res<AssetServer>,
) {
    info!("Starting asset loading...");

    game_assets.player_sprite = asset_server.load("textures/characters/Amy-Walking.png");
    game_assets.npc_nature = asset_server.load("textures/characters/Nature.png");
    game_assets.town_tileset = asset_server.load("textures/tilesets/town_tileset.png");
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
        && asset_server.is_loaded_with_dependencies(&game_assets.npc_nature)
        && asset_server.is_loaded_with_dependencies(&game_assets.town_tileset)
        && asset_server.is_loaded_with_dependencies(&game_assets.portrait_nature)
        && asset_server.is_loaded_with_dependencies(&game_assets.dialogue_font);

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
