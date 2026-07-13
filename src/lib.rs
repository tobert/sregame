// Export telemetry and instrumentation modules publicly so examples can use them.
// telemetry (tokio + OTLP/tonic exporters) cannot compile for wasm32;
// instrumentation's API surface is universal (see its module docs).
#[cfg(not(target_arch = "wasm32"))]
pub mod telemetry;
pub mod instrumentation;
