use bevy::prelude::*;
use crate::game_state::GameState;
use crate::player::Player;
use crate::dialogue::StartDialogueEvent;
use crate::assets::GameAssets;

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            check_npc_proximity,
            handle_interaction_input,
        ).chain().run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component)]
pub struct Npc {
    pub name: String,
    pub sprite_facing: NpcFacing,
}

#[derive(Clone, Copy)]
pub enum NpcFacing {
    Down = 0,
    Left = 1,
    Right = 2,
    Up = 3,
}

#[derive(Component)]
pub struct NpcDialogue {
    pub speaker: String,
    pub portrait_path: String,
    pub lines: Vec<String>,
}

#[derive(Component)]
pub struct Interactable {
    pub radius: f32,
    pub prompt: String,
}

impl Default for Interactable {
    fn default() -> Self {
        Self {
            radius: 64.0,
            prompt: "Press E to talk".to_string(),
        }
    }
}

#[derive(Component)]
struct InRange;

#[derive(Component)]
struct InteractionPrompt;

pub fn spawn_npc(
    commands: &mut Commands,
    _game_assets: &GameAssets,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    position: Vec3,
    sprite_handle: Handle<Image>,
    npc_data: Npc,
    dialogue: NpcDialogue,
) -> Entity {
    let texture = sprite_handle;

    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(48, 48),
        3,
        4,
        None,
        None,
    );
    let atlas_layout = texture_atlas_layouts.add(layout);

    let sprite_index = npc_data.sprite_facing as usize * 3 + 1;

    commands.spawn((
        npc_data,
        dialogue,
        Interactable::default(),
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: sprite_index,
            },
        ),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(2.0)),
    )).id()
}

fn check_npc_proximity(
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    npc_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, Without<InRange>)>,
    in_range_query: Query<(Entity, &Transform, &Interactable), (With<Npc>, With<InRange>)>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();

    for (entity, npc_transform, interactable) in &npc_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if distance <= interactable.radius {
            commands.entity(entity).insert(InRange);
        }
    }

    for (entity, npc_transform, interactable) in &in_range_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if distance > interactable.radius {
            commands.entity(entity).remove::<InRange>();
        }
    }
}

fn handle_interaction_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
    npc_query: Query<(&Transform, &NpcDialogue), (With<Npc>, With<InRange>)>,
    mut dialogue_events: MessageWriter<StartDialogueEvent>,
    asset_server: Res<AssetServer>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();

    let mut closest_npc: Option<(&NpcDialogue, f32)> = None;

    for (npc_transform, dialogue) in &npc_query {
        let npc_pos = npc_transform.translation.truncate();
        let distance = player_pos.distance(npc_pos);

        if let Some((_, closest_dist)) = closest_npc {
            if distance < closest_dist {
                closest_npc = Some((dialogue, distance));
            }
        } else {
            closest_npc = Some((dialogue, distance));
        }
    }

    if let Some((dialogue, _)) = closest_npc {
        let portrait = asset_server.load(&dialogue.portrait_path);

        dialogue_events.write(StartDialogueEvent {
            speaker: dialogue.speaker.clone(),
            portrait: Some(portrait),
            lines: dialogue.lines.clone(),
        });

        info!("Interacting with: {}", dialogue.speaker);
    }
}
