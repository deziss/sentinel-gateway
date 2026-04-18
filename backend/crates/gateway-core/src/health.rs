use dashmap::DashMap;
use gateway_db::models::{Backend, HealthStatus};
use reqwest::Client;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;
use uuid::Uuid;

/// Consecutive failure threshold before marking backend as unhealthy passively.
const PASSIVE_FAILURE_THRESHOLD: u32 = 5;

#[derive(Clone)]
pub struct HealthChecker {
    client: Client,
    statuses: Arc<DashMap<Uuid, HealthStatus>>,
    /// Consecutive failure counters for passive health tracking.
    failure_counts: Arc<DashMap<Uuid, AtomicU32>>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .connect_timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            statuses: Arc::new(DashMap::new()),
            failure_counts: Arc::new(DashMap::new()),
        }
    }

    pub fn statuses(&self) -> Arc<DashMap<Uuid, HealthStatus>> {
        self.statuses.clone()
    }

    pub fn get_status(&self, backend_id: Uuid) -> HealthStatus {
        self.statuses
            .get(&backend_id)
            .map(|s| s.clone())
            .unwrap_or(HealthStatus::Unknown)
    }

    /// Probe a single backend endpoint (active health check).
    pub async fn probe(&self, backend: &Backend) -> HealthStatus {
        let health_url = format!("{}/health", backend.endpoint.trim_end_matches('/'));

        match self.client.get(&health_url).send().await {
            Ok(r) if r.status().is_success() => HealthStatus::Healthy,
            Ok(r) => {
                warn!(backend_id = %backend.id, status = %r.status(), "Backend degraded");
                HealthStatus::Degraded
            }
            Err(e) => {
                warn!(backend_id = %backend.id, error = %e, "Backend unhealthy");
                HealthStatus::Unhealthy
            }
        }
    }

    /// Check all backends concurrently (active health check).
    pub async fn check_all(&self, backends: &[Backend]) {
        let futures: Vec<_> = backends.iter().map(|b| {
            let checker = self.clone();
            let backend = b.clone();
            async move {
                let status = checker.probe(&backend).await;
                let is_healthy = matches!(status, HealthStatus::Healthy);
                checker.statuses.insert(backend.id, status);
                if is_healthy {
                    // Reset passive failure counter on active healthy check
                    checker.failure_counts.remove(&backend.id);
                }
            }
        }).collect();

        futures::future::join_all(futures).await;
    }

    /// Record a successful request to a backend (passive health tracking).
    /// Resets the consecutive failure counter.
    pub fn record_success(&self, backend_id: Uuid) {
        if let Some(counter) = self.failure_counts.get(&backend_id) {
            counter.store(0, Ordering::Relaxed);
        }
        // If status was degraded/unhealthy from passive tracking, restore
        if let Some(mut status) = self.statuses.get_mut(&backend_id) {
            if matches!(*status, HealthStatus::Unhealthy | HealthStatus::Degraded) {
                *status = HealthStatus::Healthy;
            }
        }
    }

    /// Record a failed request to a backend (passive health tracking).
    /// After consecutive failures exceed the threshold, marks the backend as unhealthy.
    pub fn record_failure(&self, backend_id: Uuid) {
        let counter = self.failure_counts
            .entry(backend_id)
            .or_insert_with(|| AtomicU32::new(0));
        let count = counter.fetch_add(1, Ordering::Relaxed) + 1;

        if count >= PASSIVE_FAILURE_THRESHOLD {
            self.statuses.insert(backend_id, HealthStatus::Unhealthy);
            warn!(backend_id = %backend_id, failures = count, "Backend marked unhealthy (passive)");
        } else if count >= PASSIVE_FAILURE_THRESHOLD / 2 {
            self.statuses.insert(backend_id, HealthStatus::Degraded);
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}
