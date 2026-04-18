use gateway_db::models::Backend;
use reqwest::Method;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::{
    circuit_breaker::CircuitBreaker,
    error::CoreError,
    health::HealthChecker,
    load_balancer::LoadBalancer,
    proxy::ProxyEngine,
};

/// High-level gateway engine that orchestrates load balancing, circuit breaking,
/// health checking, and request forwarding.
///
/// This is the primary entry point for proxying requests through the gateway.
pub struct GatewayEngine {
    pub proxy: ProxyEngine,
    pub circuit_breaker: Arc<CircuitBreaker>,
    pub load_balancer: LoadBalancer,
    pub health_checker: Arc<HealthChecker>,
}

impl GatewayEngine {
    pub fn new(
        proxy: ProxyEngine,
        circuit_breaker: Arc<CircuitBreaker>,
        load_balancer: LoadBalancer,
        health_checker: Arc<HealthChecker>,
    ) -> Self {
        Self {
            proxy,
            circuit_breaker,
            load_balancer,
            health_checker,
        }
    }

    /// Select a healthy backend, forward the request, and record the outcome.
    ///
    /// Flow:
    /// 1. Filter out backends with open circuit breakers
    /// 2. Load-balance among remaining healthy backends
    /// 3. Forward the request
    /// 4. Record success/failure for circuit breaker and passive health
    /// 5. On failure, try the next backend (if available)
    pub async fn forward_to_pool(
        &self,
        backends: &[Backend],
        method: Method,
        target_path: &str,
        headers: reqwest::header::HeaderMap,
        body: bytes::Bytes,
    ) -> Result<reqwest::Response, CoreError> {
        // Filter backends with open circuit breakers
        let available: Vec<&Backend> = backends.iter()
            .filter(|b| !self.circuit_breaker.is_open(b.id))
            .collect();

        if available.is_empty() {
            return Err(CoreError::NoBackend);
        }

        // Collect into owned for load balancer (expects &[Backend])
        let available_owned: Vec<Backend> = available.iter().map(|b| (*b).clone()).collect();

        // Try up to 2 backends (primary + fallback)
        let mut last_error = CoreError::NoBackend;
        let mut tried = 0;

        while tried < 2 {
            let backend = match self.load_balancer.select(&available_owned) {
                Some(b) => b,
                None => return Err(CoreError::NoBackend),
            };

            let url = format!("{}{}", backend.endpoint.trim_end_matches('/'), target_path);
            let _guard = self.load_balancer.acquire(backend.id);

            debug!(backend_id = %backend.id, url = %url, "Forwarding request");

            match self.proxy.forward(method.clone(), &url, headers.clone(), body.clone()).await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if status < 500 {
                        self.circuit_breaker.record_success(backend.id);
                        self.health_checker.record_success(backend.id);
                    } else {
                        self.circuit_breaker.record_failure(backend.id);
                        self.health_checker.record_failure(backend.id);
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    warn!(backend_id = %backend.id, error = %e, "Backend request failed");
                    self.circuit_breaker.record_failure(backend.id);
                    self.health_checker.record_failure(backend.id);
                    last_error = e;
                    tried += 1;
                }
            }
        }

        Err(last_error)
    }
}
