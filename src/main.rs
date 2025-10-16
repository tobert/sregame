use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "The Endgame of SRE".to_string(),
                        resolution: (1920, 1080).into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                })
        )
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    info!("SRE Game initialized");
}
