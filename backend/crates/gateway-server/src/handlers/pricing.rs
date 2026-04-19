use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::tenant_pricing::UpsertTenantPricing;

/// `GET /api/v1/pricing` — list tenant pricing overrides
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    match s.tenant_pricing_repo.list_by_tenant(auth.0.tenant_id).await {
        Ok(rows) => (StatusCode::OK, Json(serde_json::json!({"pricing": rows}))).into_response(),
        Err(e) => {
            tracing::error!("List pricing failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpsertPricingRequest {
    #[validate(length(min = 1, max = 255))]
    pub model: String,
    #[validate(range(min = 0.0, max = 10000.0))]
    pub input_per_1m: Option<f64>,
    #[validate(range(min = 0.0, max = 10000.0))]
    pub output_per_1m: Option<f64>,
    #[serde(default = "default_markup")]
    #[validate(range(min = 0.0, max = 100.0))]
    pub markup_multiplier: f64,
}
fn default_markup() -> f64 { 1.0 }

/// `PUT /api/v1/pricing` — upsert a model pricing override for the tenant
pub async fn upsert(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<UpsertPricingRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let input = UpsertTenantPricing {
        tenant_id: auth.0.tenant_id,
        model: body.model,
        input_per_1m: body.input_per_1m,
        output_per_1m: body.output_per_1m,
        markup_multiplier: body.markup_multiplier,
    };

    match s.tenant_pricing_repo.upsert(input).await {
        Ok(p) => (StatusCode::OK, Json(serde_json::json!(p))).into_response(),
        Err(e) => {
            tracing::error!("Upsert pricing failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to upsert"}))).into_response()
        }
    }
}

/// `DELETE /api/v1/pricing/:model` — remove override (fall back to defaults)
pub async fn delete(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(model): Path<String>,
) -> impl IntoResponse {
    match s.tenant_pricing_repo.delete(auth.0.tenant_id, &model).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Delete pricing failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete"}))).into_response()
        }
    }
}
