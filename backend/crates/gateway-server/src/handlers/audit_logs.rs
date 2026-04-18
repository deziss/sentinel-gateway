use axum::{Json, extract::{State, Query}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub user_id: Option<Uuid>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Cursor timestamp (RFC3339). Returns rows created strictly before this time.
    pub cursor_ts: Option<chrono::DateTime<chrono::Utc>>,
    /// Cursor ID for stable ordering across rows with the same timestamp.
    pub cursor_id: Option<Uuid>,
}

fn default_limit() -> i64 { 50 }

/// `GET /api/v1/audit-logs` — cursor-paginated audit log query.
///
/// Usage:
///   GET /audit-logs?limit=50
///   GET /audit-logs?cursor_ts=2026-04-17T10:30:00Z&cursor_id=<uuid>
///
/// Response includes `next_cursor` which is `null` when no more rows.
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Query(params): Query<AuditLogQuery>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;
    let limit = params.limit.clamp(1, 200);

    match s.audit_log_repo.list_by_tenant_cursor(
        tenant_id,
        params.user_id,
        params.action,
        params.resource_type,
        params.cursor_ts,
        params.cursor_id,
        limit + 1, // fetch one extra to detect "has_more"
    ).await {
        Ok(mut logs) => {
            let has_more = logs.len() as i64 > limit;
            if has_more { logs.pop(); }

            let next_cursor = if has_more {
                logs.last().map(|last| serde_json::json!({
                    "ts": last.created_at,
                    "id": last.id,
                }))
            } else {
                None
            };

            (StatusCode::OK, Json(serde_json::json!({
                "audit_logs": logs,
                "limit": limit,
                "has_more": has_more,
                "next_cursor": next_cursor,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch audit logs: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to fetch audit logs"}))).into_response()
        }
    }
}
