//! LLM feedback endpoints.
//!
//!   POST /api/v1/feedback           → submit thumbs-up/down + comment
//!   GET  /api/v1/feedback           → list recent feedback (tenant-scoped)
//!   GET  /api/v1/feedback/stats     → aggregate counts over last N days

use axum::{
    Extension, Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::handlers::feature_gate::require_feature;
use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::llm_feedback::CreateLlmFeedback;
use gateway_license::Feature;

#[derive(Debug, Deserialize)]
pub struct SubmitFeedbackRequest {
    pub llm_log_id: Option<Uuid>,
    pub request_id: Option<String>,
    /// -1, 0, or 1.
    pub rating: i16,
    pub comment: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct SubmitFeedbackResponse {
    pub id: Uuid,
}

pub async fn submit(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<SubmitFeedbackRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_feature(&state, &auth.0, Feature::Feedback).await { return resp; }
    if !(-1..=1).contains(&body.rating) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "rating must be -1, 0, or 1"})),
        )
            .into_response();
    }
    if body.llm_log_id.is_none() && body.request_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "llm_log_id or request_id required"})),
        )
            .into_response();
    }

    let input = CreateLlmFeedback {
        tenant_id: auth.0.tenant_id,
        user_id: Some(auth.0.user_id),
        llm_log_id: body.llm_log_id,
        request_id: body.request_id,
        rating: body.rating,
        comment: body.comment,
        metadata: body.metadata,
    };

    match state.llm_feedback_repo.create(input).await {
        Ok(fb) => (StatusCode::CREATED, Json(SubmitFeedbackResponse { id: fb.id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_feature(&state, &auth.0, Feature::Feedback).await { return resp; }
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    match state
        .llm_feedback_repo
        .list_by_tenant(auth.0.tenant_id, limit)
        .await
    {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub days: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total: i64,
    pub positive: i64,
    pub negative: i64,
    pub positive_ratio: f64,
    pub window_days: i32,
}

pub async fn stats(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Query(q): Query<StatsQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_feature(&state, &auth.0, Feature::Feedback).await { return resp; }
    let days = q.days.unwrap_or(30).clamp(1, 365);
    match state.llm_feedback_repo.stats(auth.0.tenant_id, days).await {
        Ok((total, pos, neg)) => {
            let ratio = if total > 0 { pos as f64 / total as f64 } else { 0.0 };
            (
                StatusCode::OK,
                Json(StatsResponse {
                    total,
                    positive: pos,
                    negative: neg,
                    positive_ratio: ratio,
                    window_days: days,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
