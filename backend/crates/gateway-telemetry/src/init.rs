use crate::config::TelemetryConfig;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};
use tracing::{info, warn};
use tracing_subscriber::{
    Registry, EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt
};
use opentelemetry::trace::TracerProvider;

/// Initialize structured logging, tracing, and metrics. Call once at startup.
pub fn init_telemetry(config: &TelemetryConfig) {
    // ── Tracing (OTLP) ───────────────────────────────────────────────────
    let telemetry_layer = if let Some(endpoint) = config.otlp_endpoint.as_ref().filter(|e| !e.trim().is_empty()) {
        info!(endpoint = %endpoint, "Initializing OTLP tracing exporter");
        
        let exporter_builder = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint);

        match exporter_builder.build() {
            Ok(exporter) => {
                let tracer_provider = sdktrace::TracerProvider::builder()
                    .with_batch_exporter(exporter, runtime::Tokio)
                    .with_resource(Resource::new(vec![
                        KeyValue::new("service.name", config.service_name.clone()),
                        KeyValue::new("service.version", config.service_version.clone()),
                    ]))
                    .build();

                let tracer = tracer_provider.tracer("sentinel-gateway");
                global::set_tracer_provider(tracer_provider);
                Some(tracing_opentelemetry::layer().with_tracer(tracer))
            }
            Err(e) => {
                warn!(error = %e, "Failed to initialize OTLP tracer exporter; falling back to local logging");
                None
            }
        }
    } else {
        info!("Telemetry OTLP export is disabled (no endpoint configured)");
        None
    };

    // ── Logging (Stdout) ──────────────────────────────────────────────────
    let log_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_ansi(true);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let subscriber = Registry::default()
        .with(env_filter)
        .with(log_layer);

    if let Some(telemetry) = telemetry_layer {
        subscriber.with(telemetry).init();
    } else {
        subscriber.init();
    }

    info!(
        service = %config.service_name,
        version = %config.service_version,
        log_level = %config.log_level,
        prometheus = config.prometheus_enabled,
        otlp_endpoint = %config.otlp_endpoint.as_deref().unwrap_or("disabled"),
        "Telemetry system initialized"
    );
}

/// Helper to shutdown telemetry providers gracefully.
pub fn shutdown_telemetry() {
    global::shutdown_tracer_provider();
}
