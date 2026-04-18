# Observability

Sentinel Gateway emits three signals and supports three destinations:

- **Traces** — OpenTelemetry spans for every request, exportable via OTLP
- **Metrics** — Prometheus-compatible gauges, counters, histograms
- **Logs** — structured JSON via `tracing`

Destinations:

- **Built-in** — OpenTelemetry Collector (any OTLP-compatible backend: Jaeger, Tempo, Honeycomb, Datadog, ...)
- **Prometheus** — scrape `/metrics` on the gateway
- **Optional export** — forward LLM traces to Langfuse and/or Helicone (opt-in)

## Why optional external export?

OpenTelemetry is the **source of truth** for internal observability. But you might also want your LLM requests in:

- **Langfuse** — for prompt experiments, eval scoring, dataset curation
- **Helicone** — for OpenAI-style request-log UI and cost dashboards
- **Both** — each has strengths; we don't pick for you

Enabling them is **opt-in** and **fire-and-forget** — if Langfuse's API is slow or down, it never slows down your gateway.

---

## OpenTelemetry (built-in)

Enable by setting:

```bash
GATEWAY__TELEMETRY__OTLP_ENDPOINT=http://otel-collector:4317
GATEWAY__TELEMETRY__SERVICE_NAME=sentinel-gateway
GATEWAY__TELEMETRY__LOG_LEVEL=info
```

### What gets traced

Every request gets a root span. Key attributes:

| Attribute | Source |
|---|---|
| `http.method`, `http.path`, `http.status_code` | Request metadata |
| `tenant_id`, `user_id`, `api_key_id` | Auth middleware |
| `model`, `provider` | LLM handler |
| `tokens.prompt`, `tokens.completion`, `tokens.cached`, `tokens.reasoning` | Token counter |
| `cost.usd` | Cost calculator |
| `backend_id`, `backend_score` | Load balancer (inference-aware) |
| `latency_ms` | End-to-end |

Subspans:
- `policy.rate_limit` — rate limiter check
- `policy.guardrail.*` — each guardrail evaluated
- `llm.provider.call` — upstream provider call
- `db.query.*` — significant DB queries

W3C TraceContext is propagated to upstream providers via `traceparent` / `tracestate` headers — if the provider happens to support OTel, you get cross-system tracing for free.

### Sampling

By default, 100% of requests are sampled. For high-traffic deployments, configure the OTel collector to sample head-based or tail-based:

```yaml
# otel-collector.yaml
processors:
  tail_sampling:
    decision_wait: 10s
    policies:
      - name: errors
        type: status_code
        status_code: {status_codes: [ERROR]}
      - name: slow
        type: latency
        latency: {threshold_ms: 1000}
      - name: sample
        type: probabilistic
        probabilistic: {sampling_percentage: 5}
```

This keeps all errors + slow requests + 5% of normal traffic.

---

## Prometheus metrics

Scrape at `http://gateway:8080/metrics`. All metrics are prefixed `gateway_`.

### Key metrics

| Metric | Type | Labels |
|---|---|---|
| `gateway_http_requests_total` | Counter | `method`, `path`, `status` |
| `gateway_http_request_duration_seconds` | Histogram | `method`, `path`, `status` |
| `gateway_proxy_requests_total` | Counter | `tenant`, `backend`, `model`, `status` |
| `gateway_proxy_request_duration_seconds` | Histogram | `tenant`, `backend`, `model` |
| `gateway_tokens_total` | Counter | `tenant`, `model`, `direction` (input/output) |
| `gateway_cost_usd_total` | Counter | `tenant`, `model` |
| `gateway_backend_health` | Gauge | `backend_id`, `backend_name` — 1 healthy / 0 unhealthy |
| `gateway_backend_active_connections` | Gauge | `backend_id` |
| `gateway_rate_limited_total` | Counter | `tenant`, `key_type` |
| `gateway_budget_exceeded_total` | Counter | `tenant`, `scope` |
| `gateway_errors_total` | Counter | `kind` (auth, provider, internal, ...) |

### Sample PromQL queries

Request rate by model:

```promql
sum(rate(gateway_proxy_requests_total[5m])) by (model)
```

P99 latency per provider:

```promql
histogram_quantile(0.99,
  sum(rate(gateway_proxy_request_duration_seconds_bucket[5m])) by (backend, le)
)
```

Hourly cost by tenant:

```promql
sum(increase(gateway_cost_usd_total[1h])) by (tenant)
```

Tokens-per-minute by tenant+model (for TPM rate-limit dashboards):

```promql
sum(rate(gateway_tokens_total[1m])) by (tenant, model) * 60
```

### Pre-built Grafana dashboards

See `deploy/docker/grafana/` (TODO — placeholders for now). Provide your own dashboard JSONs for now.

---

## Structured logs

Logs are JSON. Controlled by `GATEWAY__TELEMETRY__LOG_LEVEL`:

```json
{
  "timestamp": "2026-04-18T14:23:45.123Z",
  "level": "info",
  "target": "gateway_server::handlers::llm",
  "message": "LLM request completed",
  "fields": {
    "tenant_id": "...",
    "user_id": "...",
    "model": "gpt-4o",
    "provider": "open_ai",
    "tokens_in": 128,
    "tokens_out": 256,
    "cost_usd": 0.00384,
    "latency_ms": 2340
  },
  "trace_id": "abc...",
  "span_id": "def..."
}
```

Trace IDs are included automatically for correlation with OTel.

### Secret redaction

The telemetry middleware **redacts sensitive headers** before logging:

- `Authorization: Bearer eyJ...` → `[REDACTED]`
- `X-API-Key: sg_...` → `[REDACTED]`
- `Cookie: ...` → `[REDACTED]`

Other sensitive patterns (passwords, tokens) in request bodies are **not** automatically redacted — don't log full request bodies at INFO.

---

## Optional: Langfuse export

Forward LLM traces to a [Langfuse](https://langfuse.com) instance (cloud or self-hosted).

```bash
GATEWAY__OBSERVABILITY__LANGFUSE__BASE_URL=https://cloud.langfuse.com
GATEWAY__OBSERVABILITY__LANGFUSE__PUBLIC_KEY=pk-lf-...
GATEWAY__OBSERVABILITY__LANGFUSE__SECRET_KEY=sk-lf-...
```

Each successful LLM request creates a Langfuse **trace** with:

- `name` = `"{provider}/{model}"`
- `userId` = gateway's user_id
- `input` = the request body (messages)
- `output` = the response body
- `metadata` = tenant_id, api_key_id, status_code, cost_usd, latency_ms

### Use cases

- **Prompt experiments** — link a Langfuse prompt version to a gateway trace.
- **Eval scoring** — run LLM-as-judge in Langfuse over gateway-produced traces.
- **Dataset curation** — promote good traces to a dataset for fine-tuning.

---

## Optional: Helicone export

Forward LLM traces to [Helicone](https://helicone.ai) via their async log API.

```bash
GATEWAY__OBSERVABILITY__HELICONE__BASE_URL=https://api.helicone.ai
GATEWAY__OBSERVABILITY__HELICONE__API_KEY=sk-helicone-...
```

Each LLM request creates a Helicone log with:

- `providerRequest` — URL, JSON body, metadata headers
- `providerResponse` — status, JSON body
- `timing` — start/end timestamps

### Use cases

- **OpenAI-style request log UI** — browse past requests, diff, replay.
- **Cost dashboards** — Helicone breaks down by user, session, custom properties.
- **Zero-markup gateway pricing** if you also use Helicone as a proxy (we just forward logs).

### Both at once

You can enable Langfuse **and** Helicone simultaneously. The exporter uses `tokio::join!` to fan out concurrently, so enabling a second destination adds no latency.

---

## Queue semantics (important)

The observability exporter:

- Uses a **bounded mpsc channel** (default 1000 events).
- Is **fire-and-forget** — `push()` never blocks, returns immediately.
- **Drops events silently on overflow** — observability backpressure must not become gateway backpressure.
- Runs on a **single background task** so it doesn't spawn unbounded tokio tasks.
- **HTTP timeout = 10s** — slow Langfuse/Helicone never stalls the gateway.

Tune the queue size via:

```bash
GATEWAY__OBSERVABILITY__QUEUE_SIZE=5000
```

Monitor drops via `gateway_observability_export_dropped_total` (future metric — currently logged at WARN level when drops happen).

---

## Troubleshooting

### "My requests are logging but not appearing in Langfuse"

Check:

1. Background task running? Look for `"Observability export enabled"` on boot.
2. Queue not full? Watch logs for `"export dropped — queue full"`.
3. Credentials correct? Test with `curl`:
   ```bash
   curl -u $PUBLIC_KEY:$SECRET_KEY \
     https://cloud.langfuse.com/api/public/ingestion \
     -H "Content-Type: application/json" \
     -d '{"batch":[]}'
   ```
   Should return 200.

### "OTel collector is getting overwhelmed"

Use tail sampling in the collector (above). The gateway has no sampling knob — it emits every span and lets the collector filter.

### "Prometheus metrics cardinality is too high"

Check which labels you're grouping by. `tenant` + `model` + `user` is fine. Adding `api_key_id` creates a new time series per key — can blow up in SaaS deployments with thousands of keys. Drop high-cardinality labels at the Prometheus recording rule level.

---

## See also

- [Architecture](../architecture.md) — where telemetry sits in the request flow
- [OpenTelemetry spec](https://opentelemetry.io/docs/)
- [Langfuse docs](https://langfuse.com/docs)
- [Helicone docs](https://docs.helicone.ai/)
