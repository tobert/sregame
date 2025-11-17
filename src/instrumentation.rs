use bevy::prelude::*;
use opentelemetry::trace::{Span as _, SpanContext, TraceContextExt, Tracer, TracerProvider as _};
use opentelemetry::metrics::{Meter, MeterProvider as _};
use opentelemetry::{global, Context as OtelContext, KeyValue};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_otlp::{MetricExporter, WithExportConfig};
use std::time::Instant;

/// Bevy resource holding the OpenTelemetry tracer
#[derive(Resource, Clone)]
pub struct GameTracer {
    tracer: opentelemetry_sdk::trace::Tracer,
}

impl GameTracer {
    pub fn tracer(&self) -> &opentelemetry_sdk::trace::Tracer {
        &self.tracer
    }
}

/// Bevy resource holding the OpenTelemetry meter for metrics
#[derive(Resource)]
pub struct GameMeter {
    meter: Meter,
    // Histograms
    pub frame_time: opentelemetry::metrics::Histogram<f64>,
    pub system_execution_time: opentelemetry::metrics::Histogram<f64>,
    pub dialogue_reading_speed: opentelemetry::metrics::Histogram<f64>,
    pub interaction_duration: opentelemetry::metrics::Histogram<f64>,

    // Counters
    pub interactions_total: opentelemetry::metrics::Counter<u64>,
    pub dialogue_lines_read: opentelemetry::metrics::Counter<u64>,
    pub map_transitions: opentelemetry::metrics::Counter<u64>,
}

/// Component attached to the player entity to track the session-level trace
/// This represents the entire play session from game start to exit
#[derive(Component)]
pub struct PlayerSessionTrace {
    pub span: opentelemetry_sdk::trace::Span,
    pub context: OtelContext,
    pub session_start: Instant,
}

impl PlayerSessionTrace {
    pub fn new(tracer: &GameTracer) -> Self {
        let mut span = tracer.tracer().start("game_session");
        span.set_attribute(KeyValue::new("session.start_time", chrono::Utc::now().to_rfc3339()));
        span.set_attribute(KeyValue::new("game.version", env!("CARGO_PKG_VERSION")));

        // Create context with current span
        let context = OtelContext::current();

        Self {
            span,
            context,
            session_start: Instant::now(),
        }
    }

    pub fn span_context(&self) -> SpanContext {
        self.span.span_context().clone()
    }

    /// Get a context that has this span set as the current span
    pub fn as_context(&self) -> OtelContext {
        OtelContext::current_with_value(self.span.span_context().clone())
    }
}

/// Component for tracking active NPC interaction
#[derive(Component)]
pub struct ActiveInteraction {
    pub span: opentelemetry_sdk::trace::Span,
    pub start_time: Instant,
    pub npc_name: String,
}

/// Resource for tracking active dialogue session
#[derive(Resource)]
pub struct ActiveDialogue {
    pub span: opentelemetry_sdk::trace::Span,
    pub start_time: Instant,
    pub speaker: String,
    pub total_lines: usize,
    pub chars_read: usize,
}

/// Initialize OpenTelemetry tracer and meter
/// Call this alongside init_telemetry() in main
/// endpoint should match the one used for logging (e.g., "http://127.0.0.1:4317")
pub fn init_instrumentation(runtime: &tokio::runtime::Runtime, endpoint: &str) -> anyhow::Result<(GameTracer, GameMeter, SdkTracerProvider, SdkMeterProvider)> {

    // Create tracer provider
    let tracer_provider = runtime.block_on(async {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()?;

        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(
                opentelemetry_sdk::Resource::builder_empty()
                    .with_service_name("sregame")
                    .build()
            )
            .build();

        Ok::<_, anyhow::Error>(provider)
    })?;

    // Set global tracer provider
    global::set_tracer_provider(tracer_provider.clone());

    // Get tracer from provider
    let tracer = tracer_provider.tracer("sregame");

    // Create meter provider with OTLP exporter
    let meter_provider = runtime.block_on(async {
        let exporter = MetricExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()?;

        let reader = PeriodicReader::builder(exporter)
            .with_interval(std::time::Duration::from_secs(10))
            .build();

        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(
                opentelemetry_sdk::Resource::builder_empty()
                    .with_service_name("sregame")
                    .build()
            )
            .build();

        Ok::<_, anyhow::Error>(provider)
    })?;

    // Set global meter provider
    global::set_meter_provider(meter_provider.clone());

    // Get meter from provider
    let meter = meter_provider.meter("sregame");

    // Create histograms
    let frame_time = meter
        .f64_histogram("game.frame_time")
        .with_description("Frame rendering time in milliseconds")
        .with_unit("ms")
        .build();

    let system_execution_time = meter
        .f64_histogram("game.system.execution_time")
        .with_description("ECS system execution time in milliseconds")
        .with_unit("ms")
        .build();

    let dialogue_reading_speed = meter
        .f64_histogram("game.dialogue.reading_speed")
        .with_description("Characters per second during dialogue reading")
        .with_unit("chars/sec")
        .build();

    let interaction_duration = meter
        .f64_histogram("game.interaction.duration")
        .with_description("Duration of player interactions in seconds")
        .with_unit("s")
        .build();

    // Create counters
    let interactions_total = meter
        .u64_counter("game.interactions.total")
        .with_description("Total number of player interactions")
        .build();

    let dialogue_lines_read = meter
        .u64_counter("game.dialogue_lines_read")
        .with_description("Total number of dialogue lines displayed")
        .build();

    let map_transitions = meter
        .u64_counter("game.map_transitions")
        .with_description("Total number of map transitions")
        .build();

    Ok((
        GameTracer { tracer },
        GameMeter {
            meter,
            frame_time,
            system_execution_time,
            dialogue_reading_speed,
            interaction_duration,
            interactions_total,
            dialogue_lines_read,
            map_transitions,
        },
        tracer_provider,
        meter_provider,
    ))
}

/// Plugin to add instrumentation resources to Bevy
pub struct InstrumentationPlugin {
    pub tracer: GameTracer,
    pub meter: GameMeter,
}

impl Plugin for InstrumentationPlugin {
    fn build(&self, _app: &mut App) {
        // Move resources into app - we need to do this carefully
        // For now, we'll initialize in main and insert manually
    }
}

/// Helper to create a span for player interactions as a child of the session span
pub fn start_interaction_span(
    tracer: &GameTracer,
    session: &PlayerSessionTrace,
    interaction_type: &str,
    player_pos: Vec2,
) -> opentelemetry_sdk::trace::Span {
    // Start span as child of session
    let context = session.as_context();
    let mut span = tracer.tracer()
        .start_with_context(
            format!("player.{}", interaction_type),
            &context,
        );

    span.set_attribute(KeyValue::new("player.x", player_pos.x as f64));
    span.set_attribute(KeyValue::new("player.y", player_pos.y as f64));
    span.set_attribute(KeyValue::new("interaction.type", interaction_type.to_string()));
    span.set_attribute(KeyValue::new("session.elapsed_ms",
        session.session_start.elapsed().as_millis() as i64));
    span
}

/// Helper to create a span for NPC interactions
pub fn start_npc_interaction_span(
    tracer: &GameTracer,
    session: &PlayerSessionTrace,
    npc_name: &str,
    player_pos: Vec2,
    distance: f32,
) -> opentelemetry_sdk::trace::Span {
    let context = session.as_context();
    let mut span = tracer.tracer()
        .start_with_context("npc.interaction", &context);

    span.set_attribute(KeyValue::new("npc.name", npc_name.to_string()));
    span.set_attribute(KeyValue::new("player.x", player_pos.x as f64));
    span.set_attribute(KeyValue::new("player.y", player_pos.y as f64));
    span.set_attribute(KeyValue::new("interaction.distance", distance as f64));
    span.set_attribute(KeyValue::new("session.elapsed_ms",
        session.session_start.elapsed().as_millis() as i64));
    span
}

/// Helper to create a span for dialogue sessions as child of interaction span
pub fn start_dialogue_span(
    tracer: &GameTracer,
    parent_context: &OtelContext,
    speaker: &str,
    line_count: usize,
) -> opentelemetry_sdk::trace::Span {
    let mut span = tracer.tracer()
        .start_with_context("dialogue.session", parent_context);

    span.set_attribute(KeyValue::new("dialogue.speaker", speaker.to_string()));
    span.set_attribute(KeyValue::new("dialogue.total_lines", line_count as i64));
    span
}

/// Helper to create a span for map transitions
pub fn start_map_transition_span(
    tracer: &GameTracer,
    session: &PlayerSessionTrace,
    from_map: &str,
    to_map: &str,
    player_pos: Vec2,
) -> opentelemetry_sdk::trace::Span {
    let context = session.as_context();
    let mut span = tracer.tracer()
        .start_with_context("map.transition", &context);

    span.set_attribute(KeyValue::new("map.from", from_map.to_string()));
    span.set_attribute(KeyValue::new("map.to", to_map.to_string()));
    span.set_attribute(KeyValue::new("player.x", player_pos.x as f64));
    span.set_attribute(KeyValue::new("player.y", player_pos.y as f64));
    span.set_attribute(KeyValue::new("session.elapsed_ms",
        session.session_start.elapsed().as_millis() as i64));
    span
}

/// Helper to add dialogue context to current span
pub fn add_dialogue_context(
    span: &mut opentelemetry_sdk::trace::Span,
    speaker: &str,
    line_number: usize,
    total_lines: usize,
) {
    span.set_attribute(KeyValue::new("dialogue.speaker", speaker.to_string()));
    span.set_attribute(KeyValue::new("dialogue.line_number", line_number as i64));
    span.set_attribute(KeyValue::new("dialogue.total_lines", total_lines as i64));
}

/// Helper to record a dialogue line event
pub fn record_dialogue_line_event(
    span: &mut opentelemetry_sdk::trace::Span,
    line_text: &str,
    index: usize,
) {
    span.add_event(
        "dialogue.line_displayed",
        vec![
            KeyValue::new("line.index", index as i64),
            KeyValue::new("line.length", line_text.len() as i64),
            KeyValue::new("line.preview", line_text.chars().take(50).collect::<String>()),
        ],
    );
}
