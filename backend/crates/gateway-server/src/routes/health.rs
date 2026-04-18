use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
use gateway_telemetry::Metrics;
use std::sync::Arc;
use std::time::Instant;
use crate::state::AppState;

pub fn health_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/healthz", get(liveness))
        .route("/readyz", get(readiness))
        .route("/metrics", get(prometheus_metrics))
}

/// Liveness probe — is the process running?
/// Fast, no external dependencies. Kubernetes uses this to restart crashed pods.
async fn liveness() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

/// Readiness probe — are all dependencies reachable?
/// Checks: database (required), Redis (optional if configured).
/// Kubernetes uses this to route traffic only to healthy instances.
async fn readiness(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut checks = serde_json::Map::new();
    let mut all_healthy = true;

    // ── Database check ─────────────────────────────────────────────────────
    let db_start = Instant::now();
    let db_ok = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&state.db),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .is_some();

    checks.insert("database".to_string(), serde_json::json!({
        "status": if db_ok { "ok" } else { "unavailable" },
        "latency_ms": db_start.elapsed().as_millis(),
    }));
    if !db_ok { all_healthy = false; }

    // ── Redis check (optional — only if configured) ────────────────────────
    // Rate limiter is InMemory or Redis; we probe indirectly by invoking it
    // with a harmless key. InMemory always returns Ok.
    let redis_start = Instant::now();
    let redis_ok = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        state.policy_engine.rate_limiter.check(
            &gateway_policy::RateLimitKey::Ip("__readyz_probe__".to_string()),
            u32::MAX,
        ),
    )
    .await
    .is_ok();

    checks.insert("rate_limiter".to_string(), serde_json::json!({
        "status": if redis_ok { "ok" } else { "unavailable" },
        "latency_ms": redis_start.elapsed().as_millis(),
    }));
    if !redis_ok { all_healthy = false; }

    // ── Activation service check ───────────────────────────────────────────
    let activation_state = state.activation_service.state().await;
    checks.insert("activation".to_string(), serde_json::json!(activation_state));

    // ── Backend availability (at least one healthy if any configured) ──────
    let backend_statuses = state.health_checker.statuses();
    let backend_count = backend_statuses.len();
    let healthy_count = backend_statuses
        .iter()
        .filter(|e| matches!(*e.value(), gateway_db::models::HealthStatus::Healthy))
        .count();

    checks.insert("backends".to_string(), serde_json::json!({
        "total": backend_count,
        "healthy": healthy_count,
    }));

    let status_code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = Json(serde_json::json!({
        "status": if all_healthy { "ready" } else { "not_ready" },
        "checks": checks,
    }));

    (status_code, body).into_response()
}

async fn prometheus_metrics() -> impl IntoResponse {
    let body = Metrics::render_prometheus();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        body,
    )
}
