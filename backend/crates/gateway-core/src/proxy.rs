use reqwest::{Client, Method, Response};
use std::time::Duration;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::CoreError;

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub default_timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub max_retries: u32,
    pub follow_redirects: bool,
    pub pool_idle_timeout_secs: u64,
    pub pool_max_idle_per_host: usize,
    pub max_body_size: usize,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 30_000,
            connect_timeout_ms: 5_000,
            max_retries: 3,
            follow_redirects: true,
            pool_idle_timeout_secs: 90,
            pool_max_idle_per_host: 256,
            max_body_size: 10 * 1024 * 1024,
        }
    }
}

/// High-throughput HTTP proxy engine with connection pooling.
///
/// Uses reqwest's connection pool with configurable keep-alive, idle timeout,
/// and per-host limits. TCP keep-alive and nodelay are enabled for low latency.
pub struct ProxyEngine {
    client: Client,
    config: ProxyConfig,
}

impl ProxyEngine {
    pub fn new(config: ProxyConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.default_timeout_ms))
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .pool_idle_timeout(Duration::from_secs(config.pool_idle_timeout_secs))
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .tcp_keepalive(Duration::from_secs(30))
            .tcp_nodelay(true)
            .http2_adaptive_window(true)
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            })
            .build()
            .expect("Failed to build HTTP client");

        Self { client, config }
    }

    pub fn config(&self) -> &ProxyConfig {
        &self.config
    }

    /// Forward a request with buffered body. Used for non-streaming requests.
    pub async fn forward(
        &self,
        method: Method,
        target_url: &str,
        headers: reqwest::header::HeaderMap,
        body: bytes::Bytes,
    ) -> Result<Response, CoreError> {
        let mut attempt = 0u32;
        loop {
            let req = self
                .client
                .request(method.clone(), target_url)
                .headers(headers.clone())
                .body(body.clone())
                .build()
                .map_err(|e| CoreError::Internal(e.to_string()))?;

            match self.client.execute(req).await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if status >= 500 && attempt < self.config.max_retries {
                        attempt += 1;
                        warn!("Backend returned {status}, retry {attempt}/{}", self.config.max_retries);
                        tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
                        continue;
                    }
                    debug!("Proxied request to {target_url} -> {status}");
                    return Ok(resp);
                }
                Err(e) if e.is_timeout() => {
                    return Err(CoreError::Timeout(target_url.to_string()));
                }
                Err(e) if e.is_connect() && attempt < self.config.max_retries => {
                    attempt += 1;
                    warn!("Connection error to {target_url}, retry {attempt}");
                    tokio::time::sleep(Duration::from_millis(200 * 2u64.pow(attempt))).await;
                    continue;
                }
                Err(e) => {
                    return Err(CoreError::ConnectionFailed(e.to_string()));
                }
            }
        }
    }

    /// Forward a request using a streaming body (no buffering).
    /// Used for large payloads, SSE, and streaming responses.
    pub async fn forward_stream(
        &self,
        method: Method,
        target_url: &str,
        headers: reqwest::header::HeaderMap,
        body: reqwest::Body,
    ) -> Result<Response, CoreError> {
        let req = self
            .client
            .request(method, target_url)
            .headers(headers)
            .body(body)
            .build()
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        self.client
            .execute(req)
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    CoreError::Timeout(target_url.to_string())
                } else if e.is_connect() {
                    CoreError::ConnectionFailed(e.to_string())
                } else {
                    CoreError::Internal(e.to_string())
                }
            })
    }

    /// Ensure a request has a X-Request-ID header. Returns the ID.
    pub fn ensure_request_id(headers: &mut reqwest::header::HeaderMap) -> String {
        if let Some(existing) = headers.get("x-request-id").and_then(|v| v.to_str().ok()) {
            return existing.to_string();
        }
        let id = Uuid::new_v4().to_string();
        if let Ok(val) = reqwest::header::HeaderValue::from_str(&id) {
            headers.insert("x-request-id", val);
        }
        id
    }
}
