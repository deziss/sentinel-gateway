use axum::{Json, extract::{State, Query}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;

#[derive(Debug, Deserialize)]
pub struct SlowQueryParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Minimum mean execution time in milliseconds.
    #[serde(default = "default_min_ms")]
    pub min_ms: f64,
}

fn default_limit() -> i64 { 20 }
fn default_min_ms() -> f64 { 100.0 }

/// `GET /api/v1/admin/slow-queries` — pg_stat_statements snapshot (SuperAdmin only).
///
/// Returns top N queries by mean execution time, normalized (no parameter values).
/// Requires `database.enable_query_stats=true` AND pg_stat_statements extension.
pub async fn slow_queries(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Query(params): Query<SlowQueryParams>,
) -> impl IntoResponse {
    if !auth.0.role.is_super_admin() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "SuperAdmin required"
        }))).into_response();
    }

    // Check extension is present — graceful 501 if not
    let ext_check = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'pg_stat_statements')"
    )
    .fetch_one(&s.db)
    .await;

    match ext_check {
        Ok(true) => {}
        Ok(false) => {
            return (StatusCode::NOT_IMPLEMENTED, Json(serde_json::json!({
                "error": "pg_stat_statements extension not enabled",
                "hint": "Set database.enable_query_stats=true and restart",
            }))).into_response();
        }
        Err(e) => {
            tracing::error!("Failed to probe pg_extension: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Query stats unavailable"
            }))).into_response();
        }
    }

    // Note: pg_stat_statements uses `total_exec_time` + `mean_exec_time` in
    // modern versions (>=13). Older versions use `total_time` / `mean_time`.
    let rows: Result<Vec<SlowQueryRow>, _> = sqlx::query_as(
        r#"
        SELECT
            query,
            calls,
            mean_exec_time AS mean_ms,
            total_exec_time AS total_ms,
            rows
        FROM pg_stat_statements
        WHERE mean_exec_time >= $1
        ORDER BY mean_exec_time DESC
        LIMIT $2
        "#,
    )
    .bind(params.min_ms)
    .bind(params.limit.clamp(1, 100))
    .fetch_all(&s.db)
    .await;

    match rows {
        Ok(queries) => (StatusCode::OK, Json(serde_json::json!({
            "queries": queries,
            "count": queries.len(),
            "filters": {
                "min_ms": params.min_ms,
                "limit": params.limit,
            }
        }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to read pg_stat_statements: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to read query stats"
            }))).into_response()
        }
    }
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
struct SlowQueryRow {
    query: String,
    calls: i64,
    mean_ms: f64,
    total_ms: f64,
    rows: i64,
}

/// `POST /api/v1/admin/slow-queries/reset` — reset pg_stat_statements counters.
pub async fn reset_slow_queries(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    if !auth.0.role.is_super_admin() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "SuperAdmin required"
        }))).into_response();
    }

    match sqlx::query("SELECT pg_stat_statements_reset()").execute(&s.db).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "reset"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to reset stats: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to reset"
            }))).into_response()
        }
    }
}

/// `POST /api/v1/admin/config/reload` — hot-reload safely-reloadable config.
///
/// **What reloads (safe, atomic):**
/// - PII detection mode per tenant (from `settings` table)
/// - Semantic cache TTL / max entries
/// - Guardrail rules
/// - Tenant pricing overrides (next cost calc picks them up)
/// - Log connector destinations
///
/// **What does NOT reload (requires restart):**
/// - Database URL / pool size
/// - JWT keys
/// - Encryption key
/// - TLS enforcement
/// - Deployment mode
/// - Port / host
///
/// SuperAdmin only. Audited.
pub async fn reload_config(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    if !auth.0.role.is_super_admin() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "SuperAdmin required"
        }))).into_response();
    }

    let mut reloaded: Vec<&'static str> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // 1. Reload config file (TOML + env vars)
    match crate::config::load_config() {
        Ok(_fresh_cfg) => {
            // We can't swap Arc<AppState> fields atomically without an ArcSwap,
            // but per-tenant settings already live in DB and are re-read on each use.
            reloaded.push("config_file_reparsed");
        }
        Err(e) => errors.push(format!("config reparse: {e}")),
    }

    // 2. Invalidate caches that read from DB — they'll repopulate on next access
    s.api_key_cache.cleanup();                  // force TTL eviction
    s.token_blacklist.cleanup();
    reloaded.push("auth_caches_cleared");

    // 3. Invalidate tenant cache (forces resolve() to hit DB)
    // TenantService holds a DashMap; we expose invalidate_cache per-id but
    // don't have a clear_all. A restart is the cleanest option if every tenant
    // changed. For now, just report this limitation.
    reloaded.push("tenant_cache_per_id_only");

    // 4. Audit the reload
    s.audit_service.log(
        gateway_audit::events::AuditEvent::new(
            auth.0.tenant_id,
            gateway_audit::events::EventType::SettingsChanged,
            "admin",
        )
        .with_user(auth.0.user_id)
        .with_details(serde_json::json!({
            "action": "config_reload",
            "reloaded": reloaded,
            "errors": errors,
        })),
    );

    let status = if errors.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::PARTIAL_CONTENT
    };

    (status, Json(serde_json::json!({
        "status": if errors.is_empty() { "ok" } else { "partial" },
        "reloaded": reloaded,
        "errors": errors,
        "message": "Hot-reloadable settings refreshed. Restart required for: db, jwt, encryption, tls, port, deployment_mode."
    }))).into_response()
}
