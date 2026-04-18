use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// OTLP endpoint, e.g. http://otel-collector:4317
    pub otlp_endpoint: Option<String>,
    /// Service name for telemetry
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Log level filter (e.g. "info,gateway_server=debug")
    pub log_level: String,
    /// Enable Prometheus metrics endpoint
    pub prometheus_enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            otlp_endpoint: None,
            service_name: "sentinel-gateway".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            log_level: "info".to_string(),
            prometheus_enabled: true,
        }
    }
}
