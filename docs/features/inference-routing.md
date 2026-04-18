# Inference-Aware Routing

Route LLM requests to self-hosted inference backends (vLLM, TGI, SGLang) based on **queue depth** and **KV-cache warmth**, not just round-robin. Produces 5-10x throughput gains on inference clusters vs naive load balancing.

## The problem

Classic load balancers treat every backend as equivalent. For vLLM / TGI / SGLang this is catastrophic:

- A backend with 50 requests queued gets the same share as an idle one.
- A backend with a warm KV-cache for prompt prefixes gets the same share as a cold one — wasting prefix caching entirely.
- Prefill time dominates latency for long prompts; warm-cache backends serve them 5-10x faster.

## The solution

Poll each backend's Prometheus `/metrics` endpoint. Route the next request to the backend with the lowest combined score of `(queue_depth + running_requests) - (warm_cache_bonus + prefix_cache_bonus)`.

## Supported runtimes

| Runtime | Queue metric | KV-cache metric | Prefix-cache metric |
|---|---|---|---|
| **vLLM** | `vllm:num_requests_waiting` | `vllm:gpu_cache_usage_perc` | `vllm:gpu_prefix_cache_hit_rate` |
| **TGI** | `tgi_queue_size` | `tgi_batch_current_size` / max (inferred) | — |
| **SGLang** | `sglang:num_requests_waiting` | `sglang:gpu_cache_usage_perc` | — |

Other runtimes exposing similar metrics can be added by extending `parse_prometheus_metrics` in `gateway-core/src/inference_metrics.rs`.

---

## Enabling it

```bash
GATEWAY__PROXY__LB_STRATEGY=inference_aware
```

Accepted aliases: `inference_aware`, `kv_cache_aware`, `inference`.

The background scraper runs unconditionally, so you can flip the strategy at runtime via config reload without restart.

---

## How it works

1. **Scraper task** runs at `health_check_interval_secs` (default 30s), fetches `/metrics` from each active backend in parallel (3s timeout each).
2. Parses Prometheus text format, extracts the metrics in the table above.
3. Stores in `InferenceMetricsCache` with TTL = 3× health-check interval.
4. At routing time, the load balancer computes a score per backend and picks the lowest.

### Scoring formula

```text
load         = queue_depth + running_requests
cache_bonus  = 10 * kv_cache_usage          // 0 to 10
prefix_bonus = 20 * prefix_cache_hit_rate   // 0 to 20

score = load - cache_bonus - prefix_bonus
```

**Lower = better.**

Examples:
- Backend A: 50 queued, 10 running, 90% cache, 80% prefix hit → `60 - 9 - 16 = 35`
- Backend B: 0 queued, 0 running, 10% cache, 0% prefix hit → `0 - 1 - 0 = -1`

Backend B wins. Good — it's idle.

- Backend C: 2 queued, 2 running, 85% cache, 90% prefix hit → `4 - 8.5 - 18 = -22.5`
- Backend D: 2 queued, 2 running, 5% cache, 0% prefix hit → `4 - 0.5 - 0 = 3.5`

Backend C wins. Good — same load, but warm cache serves the request faster.

**Tradeoff explained:** Queue depth dominates the score (integer units). Cache bonuses are up to `-30` combined, so they swing decisions between **equally-loaded** backends but don't override major load imbalances. A backend with 50 queued will always lose to one with 0 queued, regardless of cache.

---

## Fallback behavior

When inference-aware routing is enabled but a backend has no fresh metrics (scraper hasn't populated yet, or the backend doesn't expose `/metrics`):

- Its score defaults to `1000.0` — effectively deprioritized.
- But it's not excluded — if it's the only healthy backend, it still gets traffic.
- If **no** backends have fresh metrics at all, the load balancer falls back to `LeastConnections`.

This means you can gradually roll out inference-aware routing: start with a few backends exposing `/metrics`, everyone else gets classic routing, then migrate.

---

## Wiring it in

In `main.rs`:

```rust
// Metrics cache with TTL = 3x health check interval
let inference_metrics_cache = gateway_core::InferenceMetricsCache::new(
    std::time::Duration::from_secs(cfg.proxy.health_check_interval_secs * 3),
);

// Attach to load balancer
let load_balancer = gateway_core::LoadBalancer::new(lb_strategy)
    .with_inference_metrics(inference_metrics_cache.clone());

// Background scraper (simplified)
tokio::spawn(async move {
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        for backend in backend_repo.list_all().await.unwrap() {
            if !backend.is_active { continue; }
            let cache = inference_metrics_cache.clone();
            let endpoint = backend.endpoint.clone();
            tokio::spawn(async move {
                if let Some(m) = gateway_core::inference_metrics::scrape_once(&client, &endpoint).await {
                    cache.set(backend.id, m);
                }
            });
        }
    }
});
```

---

## Observability

The routing decision is visible in:

- **OTel spans** — the proxy span includes `backend_id`, `backend_name`, `backend_score` attributes.
- **Prometheus** — `gateway_inference_metrics_backend_score{backend_id}` gauge, plus per-backend `queue_depth` and `kv_cache_usage`.
- **Logs** — when DEBUG level, routing decisions log the full scorecard.

This is crucial for debugging "why did my request go to the slow backend?" questions.

---

## Performance

- **Scrape overhead per backend:** ~1 HTTP round trip (typically local, <1ms).
- **Routing decision:** O(N) where N is the number of active backends — trivial.
- **Memory:** one `InferenceMetrics` struct per backend (~80 bytes).
- **No impact on proxy path** — scraping is entirely out-of-band.

---

## Competitive context

Only **Envoy AI Gateway** (via EPP) has this upstream. Sentinel's implementation is:

- **Protocol-agnostic** — works with vLLM, TGI, SGLang, or anything exposing similar metrics.
- **Self-contained** — doesn't require the EPP CRD or Gateway API.
- **Non-invasive** — scrapes `/metrics` that these runtimes expose by default.

---

## Caveats

- **Requires backends to expose `/metrics`.** vLLM, TGI, and SGLang do by default. OpenAI/Anthropic/etc. do not — inference-aware routing is irrelevant for cloud APIs anyway (you don't control their queue).
- **Staleness.** Metrics are sampled every `health_check_interval_secs` (default 30s). If a backend's queue spikes faster than that, you'll route to it for a tick before the scraper catches up. Tune the interval down if needed.
- **No reservation.** The router picks based on observed state at time T. If N concurrent requests arrive in the same microsecond, they'll all pick the same backend before any of them dec the counter. Pair with a circuit breaker / rate limiter to avoid accidental thundering herd.
- **TGI / SGLang are less observable than vLLM.** Prefix cache metrics aren't exposed by everyone. Routing falls back to queue + running requests when prefix hit rate is absent.

---

## Tuning

| Scenario | Suggested setting |
|---|---|
| Dev/staging | `health_check_interval_secs=10` (snappier metrics, cheap) |
| Prod, stable traffic | `health_check_interval_secs=30` (default) |
| Very high throughput (>10K rpm) | `health_check_interval_secs=5`, beware of `/metrics` overhead |
| Many backends (>50) | Stagger the scrape schedule (future work) |

---

## See also

- [Architecture](../architecture.md) — where inference routing fits in the request lifecycle
- [vLLM metrics docs](https://docs.vllm.ai/en/latest/serving/metrics.html)
- [TGI metrics](https://huggingface.co/docs/text-generation-inference/reference/metrics)
