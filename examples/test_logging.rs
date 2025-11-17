// Simple example to test OTLP logging, traces, and metrics
use tracing::{info, warn, error};
use opentelemetry::KeyValue;

fn main() -> anyhow::Result<()> {
    // Get OTLP endpoint from env var or use default
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|e| {
            if e.starts_with("http://") || e.starts_with("https://") {
                e
            } else {
                format!("http://{}", e)
            }
        });

    if endpoint.is_none() {
        eprintln!("❌ OTEL_EXPORTER_OTLP_ENDPOINT not set");
        eprintln!("   Example: OTEL_EXPORTER_OTLP_ENDPOINT=127.0.0.1:4317 cargo run --example test_logging");
        anyhow::bail!("OTLP endpoint required for test_logging example");
    }

    // Initialize telemetry (logs)
    let Some((logger_provider, runtime)) = sregame::telemetry::init_telemetry(endpoint.clone())? else {
        anyhow::bail!("Telemetry initialization returned None");
    };

    info!("🔭 OpenTelemetry initialized");

    // Initialize instrumentation (traces and metrics)
    let (tracer, meter, tracer_provider, meter_provider) = sregame::instrumentation::init_instrumentation(&runtime, &endpoint.clone().unwrap())?;

    info!("📊 Instrumentation initialized");
    info!("🎮 Test example started");

    // Record some logs
    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    // Record some metrics (histograms)
    meter.frame_time.record(16.7, &[]);  // 60 FPS
    meter.frame_time.record(33.3, &[]);  // 30 FPS spike
    meter.frame_time.record(16.5, &[]);

    meter.system_execution_time.record(
        2.5,
        &[KeyValue::new("system", "player_movement")]
    );
    meter.system_execution_time.record(
        5.2,
        &[KeyValue::new("system", "tilemap_render")]
    );

    meter.dialogue_reading_speed.record(15.0, &[]);  // 15 chars/sec
    meter.dialogue_reading_speed.record(22.5, &[]);  // 22.5 chars/sec

    // Record some counters
    meter.interactions_total.add(3, &[KeyValue::new("type", "npc")]);
    meter.dialogue_lines_read.add(12, &[]);

    // Create a trace span to test traces
    use opentelemetry::trace::Tracer;
    let _span = tracer.tracer().start("test_session");

    info!("Game state: Loading");
    info!("Game state: Playing");
    info!("Player position: (10.5, 20.3)");

    // Give time for data to flush to OTLP collector
    info!("Waiting for data to flush to OTLP...");
    std::thread::sleep(std::time::Duration::from_secs(15));

    info!("🎮 Test complete, shutting down");

    // Shutdown all providers
    if let Err(e) = tracer_provider.shutdown() {
        eprintln!("Failed to shutdown tracer: {}", e);
    }
    if let Err(e) = meter_provider.shutdown() {
        eprintln!("Failed to shutdown meter: {}", e);
    }
    sregame::telemetry::shutdown_telemetry(logger_provider)?;

    // Final flush wait
    std::thread::sleep(std::time::Duration::from_secs(3));

    info!("✅ All telemetry data sent");

    Ok(())
}
