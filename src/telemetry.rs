use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, WithExportConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::Resource;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;
use anyhow::Context;

/// Initialize OpenTelemetry with OTLP exporter
/// Call this BEFORE creating the Bevy App
/// Returns Some((logger_provider, tokio_runtime)) if endpoint provided, None otherwise
pub fn init_telemetry(endpoint: Option<String>) -> anyhow::Result<Option<(SdkLoggerProvider, tokio::runtime::Runtime)>> {
    let endpoint = match endpoint {
        Some(e) => e,
        None => return Ok(None),
    };

    // Create a Tokio runtime and build the exporter within it
    let runtime = tokio::runtime::Runtime::new()
        .context("Failed to create Tokio runtime")?;

    let exporter = runtime.block_on(async {
        LogExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
    })?;

    // Create logger provider with batch processor
    let logger_provider = SdkLoggerProvider::builder()
        .with_resource(
            Resource::builder_empty()
                .with_service_name("sregame")
                .build(),
        )
        .with_batch_exporter(exporter)
        .build();

    // Create tracing layer that forwards to OTLP
    let otel_layer = OpenTelemetryTracingBridge::new(&logger_provider);

    // Filter to prevent telemetry loops
    let filter_otel = EnvFilter::new("info")
        .add_directive("hyper=off".parse().context("Failed to parse filter")?)
        .add_directive("tonic=off".parse().context("Failed to parse filter")?)
        .add_directive("h2=off".parse().context("Failed to parse filter")?)
        .add_directive("reqwest=off".parse().context("Failed to parse filter")?);

    // Also create fmt layer for local console output
    let filter_fmt = EnvFilter::new("info")
        .add_directive("opentelemetry=debug".parse().context("Failed to parse filter")?);
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_names(true)
        .with_filter(filter_fmt);

    // Initialize tracing subscriber with both layers
    tracing_subscriber::registry()
        .with(otel_layer.with_filter(filter_otel))
        .with(fmt_layer)
        .init();

    Ok(Some((logger_provider, runtime)))
}

/// Clean shutdown of telemetry
/// Call this when the app exits
pub fn shutdown_telemetry(logger_provider: SdkLoggerProvider) -> anyhow::Result<()> {
    eprintln!("🔭 Shutting down OpenTelemetry");
    logger_provider.shutdown()?;
    Ok(())
}
