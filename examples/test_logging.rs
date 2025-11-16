// Simple example to test OTLP logging without needing a display
use tracing::{info, warn, error};

fn main() -> anyhow::Result<()> {
    // Initialize telemetry
    let logger_provider = sregame::telemetry::init_telemetry()?;

    info!("🎮 Test logging example started");
    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    info!("Game state: Loading");
    info!("Game state: Playing");
    info!("Player position: (10.5, 20.3)");

    // Give time for logs to flush to OTLP collector
    std::thread::sleep(std::time::Duration::from_secs(3));

    info!("🎮 Test complete, shutting down");

    // Shutdown telemetry to flush remaining logs
    sregame::telemetry::shutdown_telemetry(logger_provider)?;

    // Wait a bit more for final flush
    std::thread::sleep(std::time::Duration::from_secs(2));

    Ok(())
}
