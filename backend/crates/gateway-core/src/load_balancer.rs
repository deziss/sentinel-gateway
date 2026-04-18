use crate::inference_metrics::InferenceMetricsCache;
use dashmap::DashMap;
use gateway_db::models::{Backend, HealthStatus};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LbStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    /// Route based on inference-server metrics (queue depth + KV cache usage).
    /// Requires `inference_metrics` to be populated by a background scraper —
    /// falls back to least-connections when no metrics are available.
    InferenceAware,
}

impl LbStrategy {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "weighted" | "weighted_round_robin" => LbStrategy::WeightedRoundRobin,
            "least_connections" | "least_conn" => LbStrategy::LeastConnections,
            "inference_aware" | "kv_cache_aware" | "inference" => LbStrategy::InferenceAware,
            _ => LbStrategy::RoundRobin,
        }
    }
}

pub struct LoadBalancer {
    strategy: LbStrategy,
    counter: Arc<Mutex<usize>>,
    active_connections: Arc<DashMap<Uuid, Arc<AtomicU64>>>,
    /// Optional — populated when inference-aware routing is enabled.
    inference_metrics: Option<InferenceMetricsCache>,
}

impl LoadBalancer {
    pub fn new(strategy: LbStrategy) -> Self {
        Self {
            strategy,
            counter: Arc::new(Mutex::new(0)),
            active_connections: Arc::new(DashMap::new()),
            inference_metrics: None,
        }
    }

    /// Attach an inference metrics cache (populated by a background scraper).
    /// Required for `LbStrategy::InferenceAware`.
    pub fn with_inference_metrics(mut self, cache: InferenceMetricsCache) -> Self {
        self.inference_metrics = Some(cache);
        self
    }

    pub fn inference_metrics(&self) -> Option<&InferenceMetricsCache> {
        self.inference_metrics.as_ref()
    }

    /// Select a healthy backend from the list.
    pub fn select<'a>(&self, backends: &'a [Backend]) -> Option<&'a Backend> {
        let healthy: Vec<&Backend> = backends
            .iter()
            .filter(|b| b.is_active)
            .filter(|b| matches!(b.health_status, HealthStatus::Healthy | HealthStatus::Unknown))
            .collect();

        if healthy.is_empty() {
            return None;
        }

        match self.strategy {
            LbStrategy::RoundRobin => {
                let mut c = self.counter.lock();
                let idx = *c % healthy.len();
                *c = c.wrapping_add(1);
                Some(healthy[idx])
            }
            LbStrategy::WeightedRoundRobin => {
                let total_weight: i32 = healthy.iter().map(|b| b.weight.max(1)).sum();
                if total_weight == 0 {
                    return healthy.first().copied();
                }
                let mut c = self.counter.lock();
                let pos = (*c as i32) % total_weight;
                *c = c.wrapping_add(1);
                let mut acc = 0i32;
                for b in &healthy {
                    acc += b.weight.max(1);
                    if pos < acc {
                        return Some(b);
                    }
                }
                Some(healthy[0])
            }
            LbStrategy::LeastConnections => {
                healthy.iter()
                    .min_by_key(|b| {
                        self.active_connections
                            .get(&b.id)
                            .map(|c| c.load(Ordering::Relaxed))
                            .unwrap_or(0)
                    })
                    .copied()
            }
            LbStrategy::InferenceAware => {
                let Some(cache) = &self.inference_metrics else {
                    // No metrics cache wired up → fall back to least-connections
                    return healthy.iter()
                        .min_by_key(|b| {
                            self.active_connections
                                .get(&b.id)
                                .map(|c| c.load(Ordering::Relaxed))
                                .unwrap_or(0)
                        })
                        .copied();
                };

                // Pick the backend with the lowest inference score. f64 isn't Ord,
                // but total_cmp gives a total ordering suitable for min_by.
                healthy.iter()
                    .min_by(|a, b| {
                        let sa = cache.score_for(a.id);
                        let sb = cache.score_for(b.id);
                        sa.total_cmp(&sb)
                    })
                    .copied()
            }
        }
    }

    /// Acquire a connection slot for a backend. Returns a guard that
    /// automatically releases the slot when dropped.
    pub fn acquire(&self, backend_id: Uuid) -> ConnectionGuard {
        let counter = self.active_connections
            .entry(backend_id)
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone();
        counter.fetch_add(1, Ordering::Relaxed);
        ConnectionGuard {
            counter,
        }
    }

    /// Get the number of active connections for a backend.
    pub fn active_count(&self, backend_id: Uuid) -> u64 {
        self.active_connections
            .get(&backend_id)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

/// RAII guard that decrements active connections when dropped.
pub struct ConnectionGuard {
    counter: Arc<AtomicU64>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}
