use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use crate::state::AppState;
use gateway_tenant::TenantContext;

pub async fn list(
    State(s): State<Arc<AppState>>,
    tenant_ctx: Option<axum::Extension<TenantContext>>,
) -> impl IntoResponse {
    let tenant_id = match tenant_ctx {
        Some(axum::Extension(ctx)) => ctx.id(),
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Tenant context missing"}))).into_response(),
    };

    match s.setting_repo.get_map(tenant_id).await {
        Ok(map) => (StatusCode::OK, Json(serde_json::json!({ "settings": map }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to list settings: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list settings"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub settings: HashMap<String, String>,
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    tenant_ctx: Option<axum::Extension<TenantContext>>,
    Json(body): Json<UpdateSettingsRequest>,
) -> impl IntoResponse {
    let tenant_id = match tenant_ctx {
        Some(axum::Extension(ctx)) => ctx.id(),
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Tenant context missing"}))).into_response(),
    };

    for (key, value) in &body.settings {
        if let Err(e) = s.setting_repo.upsert(tenant_id, key, value, false).await {
            tracing::error!("Failed to upsert setting '{key}': {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to update setting '{key}'")}))).into_response();
        }
    }

    // Return updated map
    match s.setting_repo.get_map(tenant_id).await {
        Ok(map) => (StatusCode::OK, Json(serde_json::json!({ "settings": map }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to read settings: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to read settings"}))).into_response()
        }
    }
}

pub async fn delete_key(
    State(s): State<Arc<AppState>>,
    tenant_ctx: Option<axum::Extension<TenantContext>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    let tenant_id = match tenant_ctx {
        Some(axum::Extension(ctx)) => ctx.id(),
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Tenant context missing"}))).into_response(),
    };

    match s.setting_repo.delete(tenant_id, &key).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to delete setting '{key}': {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete setting"}))).into_response()
        }
    }
}
