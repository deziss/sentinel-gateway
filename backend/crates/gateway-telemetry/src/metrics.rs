use once_cell::sync::Lazy;
use prometheus::{
    CounterVec, Gauge, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder,
};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub struct Metrics {
    pub http_requests_total: CounterVec,
    pub http_request_duration_seconds: HistogramVec,
    pub proxy_requests_total: CounterVec,
    pub proxy_request_duration_seconds: HistogramVec,
    pub tokens_total: CounterVec,
    pub cost_usd_total: CounterVec,
    pub backend_health_status: GaugeVec,
    pub active_connections: Gauge,
    pub errors_total: CounterVec,
    pub rate_limited_total: CounterVec,
    pub budget_exceeded_total: CounterVec,
}

impl Metrics {
    pub fn new() -> Self {
        let http_requests_total = CounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests"),
            &["method", "path", "status"],
        ).unwrap();

        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "HTTP request latency in seconds")
                .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
            &["method", "path"],
        ).unwrap();

        let proxy_requests_total = CounterVec::new(
            Opts::new("proxy_requests_total", "Total proxy requests"),
            &["tenant_id", "backend", "status"],
        ).unwrap();

        let proxy_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new("proxy_request_duration_seconds", "Proxy request latency in seconds")
                .buckets(vec![0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]),
            &["backend", "model"],
        ).unwrap();

        let tokens_total = CounterVec::new(
            Opts::new("tokens_total", "Total tokens processed"),
            &["tenant_id", "model", "direction"],
        ).unwrap();

        let cost_usd_total = CounterVec::new(
            Opts::new("cost_usd_total", "Total cost in USD"),
            &["tenant_id", "model"],
        ).unwrap();

        let backend_health_status = GaugeVec::new(
            Opts::new("backend_health_status", "Backend health (1=healthy, 0=unhealthy)"),
            &["tenant_id", "backend_id"],
        ).unwrap();

        let active_connections =
            Gauge::new("active_connections", "Current active proxy connections").unwrap();

        let errors_total = CounterVec::new(
            Opts::new("errors_total", "Total errors by kind"),
            &["kind", "tenant_id"],
        ).unwrap();

        let rate_limited_total = CounterVec::new(
            Opts::new("rate_limited_total", "Total rate-limited requests"),
            &["tenant_id", "key_type"],
        ).unwrap();

        let budget_exceeded_total = CounterVec::new(
            Opts::new("budget_exceeded_total", "Total budget-exceeded rejections"),
            &["tenant_id"],
        ).unwrap();

        // Register all
        let all: Vec<Box<dyn prometheus::core::Collector>> = vec![
            Box::new(http_requests_total.clone()),
            Box::new(http_request_duration_seconds.clone()),
            Box::new(proxy_requests_total.clone()),
            Box::new(proxy_request_duration_seconds.clone()),
            Box::new(tokens_total.clone()),
            Box::new(cost_usd_total.clone()),
            Box::new(backend_health_status.clone()),
            Box::new(active_connections.clone()),
            Box::new(errors_total.clone()),
            Box::new(rate_limited_total.clone()),
            Box::new(budget_exceeded_total.clone()),
        ];
        for c in all {
            REGISTRY.register(c).ok();
        }

        Self {
            http_requests_total,
            http_request_duration_seconds,
            proxy_requests_total,
            proxy_request_duration_seconds,
            tokens_total,
            cost_usd_total,
            backend_health_status,
            active_connections,
            errors_total,
            rate_limited_total,
            budget_exceeded_total,
        }
    }

    // ── Recording helpers ──────────────────────────────────────────────────

    /// Record an HTTP request (method, path, status, duration).
    pub fn record_http_request(&self, method: &str, path: &str, status: &str, duration_secs: f64) {
        self.http_requests_total.with_label_values(&[method, path, status]).inc();
        self.http_request_duration_seconds.with_label_values(&[method, path]).observe(duration_secs);
    }

    /// Record a proxy request (tenant, backend, status, duration, model).
    pub fn record_proxy_request(
        &self,
        tenant_id: &str,
        backend: &str,
        status: &str,
        duration_secs: f64,
        model: &str,
    ) {
        self.proxy_requests_total.with_label_values(&[tenant_id, backend, status]).inc();
        self.proxy_request_duration_seconds.with_label_values(&[backend, model]).observe(duration_secs);
    }

    /// Record LLM token usage.
    pub fn record_tokens(
        &self,
        tenant_id: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
    ) {
        self.tokens_total.with_label_values(&[tenant_id, model, "input"]).inc_by(tokens_in as f64);
        self.tokens_total.with_label_values(&[tenant_id, model, "output"]).inc_by(tokens_out as f64);
    }

    /// Record LLM cost.
    pub fn record_cost(&self, tenant_id: &str, model: &str, cost: f64) {
        self.cost_usd_total.with_label_values(&[tenant_id, model]).inc_by(cost);
    }

    /// Record a backend health state.
    pub fn record_backend_health(&self, tenant_id: &str, backend_id: &str, healthy: bool) {
        self.backend_health_status
            .with_label_values(&[tenant_id, backend_id])
            .set(if healthy { 1.0 } else { 0.0 });
    }

    /// Increment active connections gauge.
    pub fn inc_active_connections(&self) {
        self.active_connections.inc();
    }

    /// Decrement active connections gauge.
    pub fn dec_active_connections(&self) {
        self.active_connections.dec();
    }

    /// Record an error.
    pub fn record_error(&self, kind: &str, tenant_id: &str) {
        self.errors_total.with_label_values(&[kind, tenant_id]).inc();
    }

    /// Record a rate-limited rejection.
    pub fn record_rate_limited(&self, tenant_id: &str, key_type: &str) {
        self.rate_limited_total.with_label_values(&[tenant_id, key_type]).inc();
    }

    /// Record a budget-exceeded rejection.
    pub fn record_budget_exceeded(&self, tenant_id: &str) {
        self.budget_exceeded_total.with_label_values(&[tenant_id]).inc();
    }

    /// Render all metrics as Prometheus text exposition format.
    pub fn render_prometheus() -> String {
        let encoder = TextEncoder::new();
        let mf = REGISTRY.gather();
        encoder.encode_to_string(&mf).unwrap_or_default()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
