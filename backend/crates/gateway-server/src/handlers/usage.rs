use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, Extension};
use std::sync::Arc;
use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;

pub async fn summary(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.usage_record_repo.sum_by_tenant_30d(tenant_id).await {
        Ok((total_requests, tokens_in, tokens_out, total_cost)) => {
            (StatusCode::OK, Json(serde_json::json!({
                "period": "30d",
                "total_requests": total_requests,
                "total_tokens_input": tokens_in,
                "total_tokens_output": tokens_out,
                "total_tokens": tokens_in + tokens_out,
                "total_cost_usd": total_cost,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to query usage summary: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to query usage"
            }))).into_response()
        }
    }
}
