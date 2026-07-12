use bevy::prelude::*;
use bevy::app::ScheduleRunnerPlugin;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use bevy::window::{WindowMode, MonitorSelection};
use clap::Parser;
use std::time::Duration;

mod game_state;
mod assets;
mod character_sheet;
mod player;
mod camera;
mod tilemap;
mod dialogue;
mod npc;
mod map_data;
mod viewport;
mod semantic_state;
mod telemetry;
mod instrumentation;
mod transitions;

use game_state::{GameState, GameStatePlugin, Mode, Scene};
use assets::AssetsPlugin;
use player::PlayerPlugin;
use camera::{CameraPlugin, MainCamera, CameraFollow};
use tilemap::TilemapPlugin;
use dialogue::DialoguePlugin;
use npc::NpcPlugin;
use viewport::SemanticViewportPlugin;
use semantic_state::SemanticStatePlugin;
use transitions::TransitionsPlugin;

/// The Endgame of SRE - An educational game about Site Reliability Engineering
#[derive(Parser, Debug, Clone, Resource)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// OTLP endpoint for OpenTelemetry (e.g., 127.0.0.1:4317)
    /// If not provided, checks OTEL_EXPORTER_OTLP_ENDPOINT env var
    /// If neither is set, telemetry is disabled
    #[arg(long)]
    otlp_endpoint: Option<String>,

    /// Run with the Bevy Remote Protocol enabled (adds brp_extras methods:
    /// screenshot, send_keys, shutdown, set_window_title)
    #[arg(long)]
    remote: bool,

    /// Port for the Bevy Remote Protocol HTTP server. The default (15702) is
    /// sometimes occupied by other Bevy apps on this machine - pick another
    /// port rather than fighting over it.
    #[arg(long, default_value_t = 15702)]
    remote_port: u16,

    /// Exit the game after N frames
    #[arg(long)]
    frames: Option<u64>,

    /// Exit the game after N seconds
    #[arg(long)]
    seconds: Option<f32>,

    /// Run in headless mode (no window, no GPU required)
    /// Perfect for CI/CD, automated testing, and environments without display servers.
    /// For GPU-rendered headless (real frames, screenshots), see scripts/run-headless.sh
    #[arg(long)]
    headless: bool,

    /// Run in borderless fullscreen instead of a 1920x1080 window
    #[arg(long)]
    fullscreen: bool,

    /// OTLP metric export interval in milliseconds (default: 10000)
    #[arg(long)]
    otlp_metric_interval: Option<u64>,
}

fn main() {
    let args = Args::parse();

    // Determine OTLP endpoint: CLI flag takes precedence over env var
    let otlp_endpoint = args.otlp_endpoint.clone()
        .or_else(|| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok())
        .map(|e| {
            // Add http:// prefix if not present
            if e.starts_with("http://") || e.starts_with("https://") {
                e
            } else {
                format!("http://{}", e)
            }
        });

    // Initialize OpenTelemetry BEFORE Bevy app
    // This sets up the tracing subscriber before Bevy's LogPlugin does
    let telemetry_result = telemetry::init_telemetry(otlp_endpoint.clone());
    let (logger_provider, runtime, tracer, meter, tracer_provider, meter_provider) = match telemetry_result {
        Ok(Some((logger, runtime))) => {
            eprintln!("🔭 OpenTelemetry enabled: {}", otlp_endpoint.as_ref().unwrap());
            info!("🔭 OpenTelemetry initialized, sending logs to OTLP collector");

            // Initialize instrumentation (traces and metrics)
            match instrumentation::init_instrumentation(
                &runtime, 
                otlp_endpoint.as_ref().unwrap(),
                args.otlp_metric_interval
            ) {
                Ok((tracer, meter, tracer_prov, meter_prov)) => {
                    info!("📊 Instrumentation initialized with traces and metrics");
                    (Some(logger), Some(runtime), Some(tracer), Some(meter), Some(tracer_prov), Some(meter_prov))
                }
                Err(e) => {
                    eprintln!("⚠️  Instrumentation unavailable, continuing without traces/metrics: {}", e);
                    info!("⚠️  Instrumentation unavailable, continuing without traces/metrics");
                    (Some(logger), Some(runtime), None, None, None, None)
                }
            }
        }
        Ok(None) => {
            eprintln!("ℹ️  OpenTelemetry disabled (no endpoint configured)");
            eprintln!("   Use --otlp-endpoint or OTEL_EXPORTER_OTLP_ENDPOINT to enable");
            // Fall back to basic console logging
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();
            (None, None, None, None, None, None)
        }
        Err(e) => {
            eprintln!("⚠️  OpenTelemetry unavailable: {}", e);
            eprintln!("   Continuing with console-only logging");
            // Fall back to basic console logging
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();
            (None, None, None, None, None, None)
        }
    };

    let mut app = App::new();

    if args.headless {
        info!("🔧 Running in headless mode (no window, no display server required)");
        app.add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>()
                .disable::<bevy::log::LogPlugin>()
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(
            Duration::from_secs_f64(1.0 / 60.0)
        ));
    } else {
        let window_mode = if args.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };

        app.add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "The Endgame of SRE".to_string(),
                        resolution: (1920, 1080).into(),
                        resizable: false,
                        mode: window_mode,
                        ..default()
                    }),
                    ..default()
                })
                .disable::<bevy::log::LogPlugin>()
        );
    }

    if args.remote {
        app.add_plugins(bevy_brp_extras::BrpExtrasPlugin::with_port(args.remote_port));
    }

    // Insert CLI args as resource
    app.insert_resource(args.clone());

    // Insert telemetry resources if available
    if let Some(t) = tracer {
        app.insert_resource(t);
    }
    if let Some(m) = meter {
        app.insert_resource(m);
    }

    app
        .add_plugins((
            GameStatePlugin,
            AssetsPlugin,
            PlayerPlugin,
            CameraPlugin,
            TilemapPlugin,
            DialoguePlugin,
            NpcPlugin,
            SemanticViewportPlugin,
            SemanticStatePlugin,
            TransitionsPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Playing), on_enter_playing)
        .add_systems(OnEnter(Mode::Dialogue), on_enter_dialogue)
        .add_systems(Update, exit_after_n_frames_or_seconds)
        .run();

    // Shutdown telemetry when app exits
    info!("Shutting down instrumentation providers");
    if let Some(tp) = tracer_provider {
        if let Err(e) = tp.shutdown() {
            eprintln!("Failed to shutdown tracer: {}", e);
        }
    }
    if let Some(mp) = meter_provider {
        if let Err(e) = mp.shutdown() {
            eprintln!("Failed to shutdown meter: {}", e);
        }
    }
    if let Some(lp) = logger_provider {
        if let Err(e) = telemetry::shutdown_telemetry(lp) {
            eprintln!("Failed to shutdown logger: {}", e);
        }
    }

    // Keep runtime alive for final flush if telemetry was active
    if runtime.is_some() {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        CameraFollow::default(),
        Transform::from_xyz(0.0, 0.0, 999.9),
    ));

    info!("SRE Game initialized");
}

fn on_enter_playing(mut next_scene: ResMut<NextState<Scene>>) {
    info!("Entering Playing state - player can explore");
    next_scene.set(Scene::TownOfEndgame);
}

fn on_enter_dialogue() {
    info!("Entering Dialogue state - reading conversation");
}

fn exit_after_n_frames_or_seconds(
    args: Res<Args>,
    time: Res<Time>,
    mut frame_count: Local<u64>,
    mut exit: MessageWriter<bevy::app::AppExit>,
) {
    *frame_count += 1;
    let elapsed = time.elapsed_secs_f64() as f32;

    if let Some(frames) = args.frames {
        if *frame_count >= frames {
            info!("Reached target frame count ({frames}), exiting.");
            exit.write(bevy::app::AppExit::Success);
        }
    }

    if let Some(seconds) = args.seconds {
        if elapsed >= seconds {
            info!("Reached target duration ({seconds}s), exiting.");
            exit.write(bevy::app::AppExit::Success);
        }
    }
}
