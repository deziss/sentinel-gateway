use crate::config::TelemetryConfig;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use opentelemetry::trace::TracerProvider;

/// Initialize structured logging, tracing, and metrics. Call once at startup.
pub fn init_telemetry(config: &TelemetryConfig) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_current_span(true);

    // Resource attributes
    let resource = Resource::new(vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service.version", config.service_version.clone()),
        KeyValue::new("deployment.environment", "production"),
    ]);

    // OTLP Tracer (Conditional)
    let telemetry_layer = if let Some(endpoint) = &config.otlp_endpoint {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("Failed to create OTLP span exporter");

        let tracer_provider = sdktrace::TracerProvider::builder()
            .with_batch_exporter(exporter, runtime::Tokio)
            .with_resource(resource)
            .build();

        let tracer = tracer_provider.tracer("sentinel-gateway");
        global::set_tracer_provider(tracer_provider);

        Some(tracing_opentelemetry::layer().with_tracer(tracer))
    } else {
        None
    };

    // Setup W3C TraceContext propagator
    global::set_text_map_propagator(opentelemetry_sdk::propagation::TraceContextPropagator::new());

    // Explicitly type the registry to help inference
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    if let Some(telemetry) = telemetry_layer {
        subscriber.with(telemetry).init();
    } else {
        subscriber.init();
    }

    tracing::info!(
        service = %config.service_name,
        version = %config.service_version,
        endpoint = %config.otlp_endpoint.as_deref().unwrap_or("disabled"),
        "Telemetry initialized"
    );
}

/// Helper to shutdown telemetry providers gracefully.
pub fn shutdown_telemetry() {
    global::shutdown_tracer_provider();
}
