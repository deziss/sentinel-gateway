use axum::{Json, extract::{State, Query}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;

#[derive(Debug, Deserialize)]
pub struct LlmLogsQuery {
    pub user_id: Option<Uuid>,
    pub model: Option<String>,
    /// Filter by status code range
    pub status_min: Option<i32>,
    pub status_max: Option<i32>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor_ts: Option<chrono::DateTime<chrono::Utc>>,
    pub cursor_id: Option<Uuid>,
}

fn default_limit() -> i64 { 50 }

/// `GET /api/v1/llm-logs` — search captured LLM requests/responses.
///
/// Cursor-paginated. Filters: user_id, model, status code range.
/// Examples:
///   GET /llm-logs?model=gpt-4o&status_min=500      # errors only
///   GET /llm-logs?user_id=<uuid>&limit=100         # per-user
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Query(params): Query<LlmLogsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.clamp(1, 200);

    match s.llm_log_repo.search(
        auth.0.tenant_id,
        params.user_id,
        params.model,
        params.status_min,
        params.status_max,
        params.cursor_ts,
        params.cursor_id,
        limit + 1,
    ).await {
        Ok(mut logs) => {
            let has_more = logs.len() as i64 > limit;
            if has_more { logs.pop(); }

            let next_cursor = if has_more {
                logs.last().map(|l| serde_json::json!({
                    "ts": l.created_at,
                    "id": l.id,
                }))
            } else {
                None
            };

            (StatusCode::OK, Json(serde_json::json!({
                "llm_logs": logs,
                "limit": limit,
                "has_more": has_more,
                "next_cursor": next_cursor,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("LLM logs search failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to search LLM logs"
            }))).into_response()
        }
    }
}
