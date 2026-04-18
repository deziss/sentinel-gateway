//! HTTP/3 support stub.
//!
//! HTTP/3 runs over QUIC (via quinn + h3 crates). Axum 0.7 does not natively
//! support HTTP/3, so this requires a separate QUIC listener on a dedicated port.
//!
//! This module provides the configuration interface. Full implementation
//! will be added when the Rust HTTP/3 ecosystem stabilizes.

use serde::{Deserialize, Serialize};

/// HTTP/3 listener configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http3Config {
    pub enabled: bool,
    /// Separate port for QUIC (default 8443).
    pub port: u16,
    /// Path to TLS certificate (PEM). Required for QUIC.
    pub cert_path: String,
    /// Path to TLS private key (PEM). Required for QUIC.
    pub key_path: String,
}

impl Default for Http3Config {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8443,
            cert_path: "certs/server.pem".to_string(),
            key_path: "certs/server.key".to_string(),
        }
    }
}

/// Placeholder for HTTP/3 server. Logs a warning and returns.
pub async fn serve_http3(_config: Http3Config) -> anyhow::Result<()> {
    tracing::warn!("HTTP/3 support is not yet implemented. Requires quinn + h3 crates.");
    Ok(())
}
