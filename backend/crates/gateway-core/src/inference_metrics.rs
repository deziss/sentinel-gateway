//! Inference-server-aware routing metrics.
//!
//! Polls Prometheus-format `/metrics` endpoints on self-hosted inference backends
//! (vLLM, TGI, SGLang) and caches the last-known queue depth + KV-cache usage.
//! The load balancer then prefers backends with shorter queues and warmer caches,
//! which can produce 5-10x throughput gains over naive round-robin on inference clusters.
//!
//! # Supported runtimes
//!
//! | Runtime | Queue metric | KV-cache metric | Prefix-cache metric |
//! |---------|-------------|-----------------|---------------------|
//! | vLLM    | `vllm:num_requests_waiting` | `vllm:gpu_cache_usage_perc` | `vllm:gpu_prefix_cache_hit_rate` |
//! | TGI     | `tgi_queue_size`            | `tgi_batch_current_size` / max | — |
//! | SGLang  | `sglang:num_requests_running` | `sglang:gpu_cache_usage_perc` | — |
//!
//! # Design notes
//!
//! - **Poll out-of-band.** A dedicated task scrapes `/metrics` on the interval
//!   (default 10s) — we never block the request path waiting for fresh metrics.
//! - **Stale data is fine.** If the scraper is behind, we route on the last-known
//!   values. The LB cost is consistent even when metrics are slightly stale.
//! - **Fail-open.** If a backend doesn't expose metrics, it gets a neutral score
//!   so it still receives traffic (just scored worse than one with good metrics).

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Snapshot of inference-server metrics for a single backend.
#[derive(Debug, Clone, Default)]
pub struct InferenceMetrics {
    /// Requests waiting in queue (not yet started). Lower = better.
    pub queue_depth: u32,
    /// KV-cache utilisation, 0.0 to 1.0. Higher = warmer cache = faster time-to-first-token.
    pub kv_cache_usage: f32,
    /// Prefix-cache hit rate, 0.0 to 1.0. Higher = better (more tokens served from cache).
    /// vLLM only; 0 for runtimes that don't expose it.
    pub prefix_cache_hit_rate: f32,
    /// Requests currently being generated. Combined with queue_depth = effective load.
    pub running_requests: u32,
    /// When these metrics were last scraped.
    pub scraped_at: Option<Instant>,
}

impl InferenceMetrics {
    /// Routing score — lower is better. The LB picks the backend with the lowest score.
    ///
    /// Heuristic: queue depth dominates (you can't serve requests stuck in a queue),
    /// but among backends with empty queues we prefer warmer caches.
    ///
    /// `total_load = queue_depth + running_requests`
    /// `score = total_load - 10 * kv_cache_usage - 20 * prefix_cache_hit_rate`
    ///
    /// So a backend serving 5 requests with a 90% warm cache beats an idle backend
    /// with a cold cache. But 50 queued requests always lose, cache or no cache.
    pub fn routing_score(&self) -> f64 {
        let load = (self.queue_depth + self.running_requests) as f64;
        let cache_bonus = 10.0 * self.kv_cache_usage as f64;
        let prefix_bonus = 20.0 * self.prefix_cache_hit_rate as f64;
        load - cache_bonus - prefix_bonus
    }

    /// How old are these metrics? Used by the LB to decide whether to trust them.
    pub fn age(&self) -> Option<Duration> {
        self.scraped_at.map(|t| t.elapsed())
    }

    /// Fresh if scraped within `ttl`. Stale data gets a neutral score.
    pub fn is_fresh(&self, ttl: Duration) -> bool {
        self.age().map(|a| a <= ttl).unwrap_or(false)
    }
}

/// Cache of latest inference metrics, keyed by backend ID.
#[derive(Clone)]
pub struct InferenceMetricsCache {
    inner: Arc<DashMap<Uuid, InferenceMetrics>>,
    /// TTL — metrics older than this are considered stale.
    ttl: Duration,
}

impl InferenceMetricsCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, backend_id: Uuid) -> Option<InferenceMetrics> {
        self.inner.get(&backend_id).map(|m| m.clone())
    }

    pub fn set(&self, backend_id: Uuid, metrics: InferenceMetrics) {
        self.inner.insert(backend_id, metrics);
    }

    pub fn remove(&self, backend_id: Uuid) {
        self.inner.remove(&backend_id);
    }

    /// Returns the score to use for routing. Fresh metrics → their score;
    /// stale or missing → a large score so they get deprioritised (but not excluded).
    pub fn score_for(&self, backend_id: Uuid) -> f64 {
        match self.inner.get(&backend_id) {
            Some(m) if m.is_fresh(self.ttl) => m.routing_score(),
            _ => 1_000.0, // stale/missing → deprioritise but don't exclude
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for InferenceMetricsCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

// ── Prometheus metrics scraper ───────────────────────────────────────────────

/// Parse vLLM / TGI / SGLang Prometheus-format metrics text and extract
/// inference-relevant gauges.
///
/// Prometheus format is line-oriented:
/// ```text
/// # HELP vllm:num_requests_waiting ...
/// # TYPE vllm:num_requests_waiting gauge
/// vllm:num_requests_waiting{model_name="..."} 5.0
/// ```
/// We grep the metric name prefix and take the latest non-comment value.
pub fn parse_prometheus_metrics(body: &str) -> InferenceMetrics {
    let mut m = InferenceMetrics::default();

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split at first whitespace — Prometheus lines are `metric{labels} value [timestamp]`
        let Some((metric_and_labels, rest)) = line.split_once(|c: char| c.is_whitespace()) else {
            continue;
        };
        let value_str = rest.split_whitespace().next().unwrap_or("");
        let Ok(value) = value_str.parse::<f64>() else {
            continue;
        };

        // Strip labels: `metric{foo="bar"}` -> `metric`
        let metric = metric_and_labels
            .split_once('{')
            .map(|(m, _)| m)
            .unwrap_or(metric_and_labels);

        match metric {
            // vLLM
            "vllm:num_requests_waiting" => m.queue_depth = value as u32,
            "vllm:num_requests_running" => m.running_requests = value as u32,
            "vllm:gpu_cache_usage_perc" => m.kv_cache_usage = value as f32,
            "vllm:gpu_prefix_cache_hit_rate" => m.prefix_cache_hit_rate = value as f32,
            // TGI
            "tgi_queue_size" => m.queue_depth = value as u32,
            "tgi_batch_current_size" => m.running_requests = value as u32,
            // SGLang
            "sglang:num_requests_waiting" => m.queue_depth = value as u32,
            "sglang:num_requests_running" => m.running_requests = value as u32,
            "sglang:gpu_cache_usage_perc" => m.kv_cache_usage = value as f32,
            _ => {}
        }
    }

    m.scraped_at = Some(Instant::now());
    m
}

/// Scrape a single backend's `/metrics` endpoint.
pub async fn scrape_once(client: &reqwest::Client, endpoint: &str) -> Option<InferenceMetrics> {
    let url = format!("{}/metrics", endpoint.trim_end_matches('/'));
    match client.get(&url).timeout(Duration::from_secs(3)).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.text().await {
                Ok(body) => Some(parse_prometheus_metrics(&body)),
                Err(e) => {
                    tracing::debug!(endpoint, error = %e, "Failed to read metrics body");
                    None
                }
            }
        }
        Ok(resp) => {
            tracing::debug!(endpoint, status = resp.status().as_u16(), "Metrics endpoint non-200");
            None
        }
        Err(e) => {
            tracing::debug!(endpoint, error = %e, "Metrics endpoint unreachable");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vllm_metrics() {
        let body = r#"
# HELP vllm:num_requests_waiting Number of requests waiting
# TYPE vllm:num_requests_waiting gauge
vllm:num_requests_waiting{model_name="llama-3-70b"} 3.0
vllm:num_requests_running{model_name="llama-3-70b"} 8.0
vllm:gpu_cache_usage_perc{model_name="llama-3-70b"} 0.72
vllm:gpu_prefix_cache_hit_rate{model_name="llama-3-70b"} 0.45
"#;
        let m = parse_prometheus_metrics(body);
        assert_eq!(m.queue_depth, 3);
        assert_eq!(m.running_requests, 8);
        assert!((m.kv_cache_usage - 0.72).abs() < 0.001);
        assert!((m.prefix_cache_hit_rate - 0.45).abs() < 0.001);
        assert!(m.scraped_at.is_some());
    }

    #[test]
    fn parse_tgi_metrics() {
        let body = r#"
tgi_queue_size 12
tgi_batch_current_size 4
"#;
        let m = parse_prometheus_metrics(body);
        assert_eq!(m.queue_depth, 12);
        assert_eq!(m.running_requests, 4);
    }

    #[test]
    fn routing_score_prefers_empty_queue() {
        let busy = InferenceMetrics {
            queue_depth: 50,
            running_requests: 10,
            kv_cache_usage: 0.9,
            prefix_cache_hit_rate: 0.8,
            scraped_at: Some(Instant::now()),
        };
        let idle = InferenceMetrics {
            queue_depth: 0,
            running_requests: 0,
            kv_cache_usage: 0.1,
            prefix_cache_hit_rate: 0.0,
            scraped_at: Some(Instant::now()),
        };
        // busy has high load → should have a much higher score than idle
        assert!(busy.routing_score() > idle.routing_score());
    }

    #[test]
    fn routing_score_warm_cache_beats_cold_cache_when_load_equal() {
        let warm = InferenceMetrics {
            queue_depth: 2,
            running_requests: 2,
            kv_cache_usage: 0.85,
            prefix_cache_hit_rate: 0.9,
            scraped_at: Some(Instant::now()),
        };
        let cold = InferenceMetrics {
            queue_depth: 2,
            running_requests: 2,
            kv_cache_usage: 0.05,
            prefix_cache_hit_rate: 0.0,
            scraped_at: Some(Instant::now()),
        };
        assert!(warm.routing_score() < cold.routing_score());
    }

    #[test]
    fn cache_deprioritises_missing_backends() {
        let cache = InferenceMetricsCache::new(Duration::from_secs(30));
        let a = Uuid::new_v4();
        cache.set(
            a,
            InferenceMetrics {
                queue_depth: 1,
                running_requests: 1,
                kv_cache_usage: 0.5,
                prefix_cache_hit_rate: 0.5,
                scraped_at: Some(Instant::now()),
            },
        );
        let b = Uuid::new_v4(); // never set → stale path
        assert!(cache.score_for(a) < cache.score_for(b));
    }

    #[test]
    fn cache_treats_stale_metrics_as_low_priority() {
        let cache = InferenceMetricsCache::new(Duration::from_millis(1));
        let id = Uuid::new_v4();
        cache.set(
            id,
            InferenceMetrics {
                queue_depth: 0,
                kv_cache_usage: 1.0,
                prefix_cache_hit_rate: 1.0,
                running_requests: 0,
                scraped_at: Some(Instant::now() - Duration::from_secs(10)),
            },
        );
        // Metrics are older than the 1ms TTL → should be neutral score, not the computed -30
        assert_eq!(cache.score_for(id), 1_000.0);
    }
}
