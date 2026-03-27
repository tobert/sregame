use bevy::prelude::*;
use opentelemetry::trace::{Span as _, SpanContext, Tracer, TracerProvider as _};
use opentelemetry::metrics::MeterProvider as _;
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
    pub dialogue_reading_speed: opentelemetry::metrics::Histogram<f64>,
    pub interactions_total: opentelemetry::metrics::Counter<u64>,
    pub dialogue_lines_read: opentelemetry::metrics::Counter<u64>,
}

/// Component attached to the player entity to track the session-level trace
/// This represents the entire play session from game start to exit
#[derive(Component)]
pub struct PlayerSessionTrace {
    pub span: opentelemetry_sdk::trace::Span,
    pub session_start: Instant,
}

impl PlayerSessionTrace {
    pub fn new(tracer: &GameTracer) -> Self {
        let mut span = tracer.tracer().start("game_session");
        span.set_attribute(KeyValue::new("session.start_time", chrono::Utc::now().to_rfc3339()));
        span.set_attribute(KeyValue::new("game.version", env!("CARGO_PKG_VERSION")));

        Self {
            span,
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

/// Resource for tracking active dialogue session
#[derive(Resource)]
pub struct ActiveDialogue {
    pub span: opentelemetry_sdk::trace::Span,
    pub start_time: Instant,
    pub speaker: String,
    pub chars_read: usize,
}

/// Initialize OpenTelemetry tracer and meter
/// Call this alongside init_telemetry() in main
/// endpoint should match the one used for logging (e.g., "http://127.0.0.1:4317")
/// metric_interval_ms is the export interval in milliseconds (default: 10000ms)
pub fn init_instrumentation(
    runtime: &tokio::runtime::Runtime, 
    endpoint: &str,
    metric_interval_ms: Option<u64>
) -> anyhow::Result<(GameTracer, GameMeter, SdkTracerProvider, SdkMeterProvider)> {

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

        let interval = std::time::Duration::from_millis(metric_interval_ms.unwrap_or(10000));

        let reader = PeriodicReader::builder(exporter)
            .with_interval(interval)
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

    let dialogue_reading_speed = meter
        .f64_histogram("game.dialogue.reading_speed")
        .with_description("Characters per second during dialogue reading")
        .with_unit("chars/sec")
        .build();

    let interactions_total = meter
        .u64_counter("game.interactions.total")
        .with_description("Total number of player interactions")
        .build();

    let dialogue_lines_read = meter
        .u64_counter("game.dialogue_lines_read")
        .with_description("Total number of dialogue lines displayed")
        .build();

    Ok((
        GameTracer { tracer },
        GameMeter {
            dialogue_reading_speed,
            interactions_total,
            dialogue_lines_read,
        },
        tracer_provider,
        meter_provider,
    ))
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
