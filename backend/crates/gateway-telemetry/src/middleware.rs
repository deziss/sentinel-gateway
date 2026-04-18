use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use std::sync::Arc;
use std::time::Instant;

use crate::metrics::Metrics;

/// Axum middleware that:
/// 1. Creates tracing spans with gateway-specific attributes
/// 2. Injects security headers on every response
/// 3. Logs request completion
pub async fn telemetry_middleware(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let tenant_id = req.extensions()
        .get::<gateway_tenant::TenantContext>()
        .map(|ctx| ctx.id().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let user_id = req.extensions()
        .get::<gateway_auth::middleware::RequireAuth>()
        .map(|auth| auth.0.user_id.to_string())
        .unwrap_or_else(|| "anonymous".to_string());

    let span = tracing::info_span!(
        "http_request",
        http.method = %method,
        http.path = %path,
        tenant_id = %tenant_id,
        user_id = %user_id,
        http.status_code = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
    );

    let _guard = span.enter();

    let mut response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status().as_u16();

    span.record("http.status_code", status);
    span.record("duration_ms", duration.as_millis() as u64);

    // ── Security headers ───────────────────────────────────────────────────
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("x-xss-protection", HeaderValue::from_static("1; mode=block"));
    headers.insert("referrer-policy", HeaderValue::from_static("strict-origin-when-cross-origin"));
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );

    tracing::info!(
        method = %method,
        path = %path,
        status = status,
        duration_ms = duration.as_millis() as u64,
        tenant_id = %tenant_id,
        "request completed"
    );

    response
}

/// TLS enforcement configuration.
#[derive(Debug, Clone)]
pub struct TlsEnforcement {
    pub require_tls: bool,
    pub trust_forwarded_proto: bool,
}

/// Middleware that rejects plaintext HTTP requests when TLS is required.
///
/// Inspects `X-Forwarded-Proto` (trusted reverse proxy) and the URI scheme.
/// Returns `426 Upgrade Required` with `Upgrade: TLS/1.2` header on plaintext.
pub async fn tls_enforcement_middleware(
    axum::extract::State(cfg): axum::extract::State<std::sync::Arc<TlsEnforcement>>,
    req: Request,
    next: Next,
) -> Response {
    if !cfg.require_tls {
        return next.run(req).await;
    }

    // Skip enforcement for localhost/health-check paths (Kubernetes probes)
    let path = req.uri().path();
    if path == "/healthz" || path == "/readyz" {
        return next.run(req).await;
    }

    // Check X-Forwarded-Proto header if trusted
    let is_tls = if cfg.trust_forwarded_proto {
        req.headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("https"))
            .unwrap_or(false)
    } else {
        // Fallback: check URI scheme (only works with absolute URIs)
        req.uri().scheme_str() == Some("https")
    };

    if !is_tls {
        return axum::response::IntoResponse::into_response((
            axum::http::StatusCode::UPGRADE_REQUIRED,
            [
                ("upgrade", "TLS/1.2, HTTP/1.1"),
                ("connection", "Upgrade"),
                ("content-type", "application/json"),
            ],
            r#"{"error":"TLS required","message":"This endpoint requires HTTPS"}"#,
        ));
    }

    let mut response = next.run(req).await;
    // Add HSTS on successful TLS responses
    response.headers_mut().insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    response
}

/// Axum middleware that records Prometheus metrics using a shared Metrics instance.
pub async fn metrics_middleware(
    axum::extract::State(metrics): axum::extract::State<Arc<Metrics>>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics.record_http_request(&method, &path, &status, duration);

    response
}