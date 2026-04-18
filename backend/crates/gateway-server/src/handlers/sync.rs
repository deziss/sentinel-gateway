use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_tenant::sync::{
    PlatformRegistrationRequest, PlatformSyncService, SyncPushPayload,
};

pub async fn status(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let svc = PlatformSyncService::new(s.setting_repo.clone(), auth.0.tenant_id);

    match svc.get_state().await {
        Ok(state) => {
            (StatusCode::OK, Json(serde_json::json!({
                "sync": state,
                "deployment_mode": s.deployment_mode,
                "current_plan": s.features.plan,
                "instance_id": s.server_config.instance_id,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get sync state: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(url(message = "Must be a valid platform URL"))]
    pub platform_url: String,
    #[validate(email(message = "Must be a valid email address"))]
    pub admin_email: String,
    #[validate(length(max = 255))]
    pub instance_name: Option<String>,
}

pub async fn register(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;
    let svc = PlatformSyncService::new(s.setting_repo.clone(), tenant_id);

    let instance_id = s.server_config.instance_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let instance_name = body.instance_name
        .or_else(|| s.server_config.instance_name.clone())
        .unwrap_or_else(|| "Sentinel Gateway".to_string());

    let user_count = s.user_repo.count_by_tenant(tenant_id).await.unwrap_or(0);
    let backend_count = s.backend_repo.count_by_tenant(tenant_id).await.unwrap_or(0);
    let api_key_count = s.api_key_repo.count_by_tenant(tenant_id).await.unwrap_or(0);

    let req = PlatformRegistrationRequest {
        instance_id,
        instance_name,
        admin_email: body.admin_email,
        current_plan: format!("{:?}", s.features.plan).to_lowercase(),
        user_count,
        backend_count,
        api_key_count,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    match svc.register(&body.platform_url, req).await {
        Ok(resp) => {
            (StatusCode::OK, Json(serde_json::json!({
                "status": "linked",
                "platform_tenant_id": resp.platform_tenant_id,
                "plan": resp.plan,
                "message": "Instance linked successfully"
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Sync register failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn push(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;
    let svc = PlatformSyncService::new(s.setting_repo.clone(), tenant_id);

    let state = match svc.get_state().await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    let (platform_url, api_key) = match &state {
        gateway_tenant::sync::SyncState::Linked { platform_url, .. } => {
            let map = s.setting_repo.get_map(tenant_id).await.unwrap_or_default();
            let key = map.get("sync_api_key").cloned().unwrap_or_default();
            (platform_url.clone(), key)
        }
        _ => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Not linked to platform"}))).into_response(),
    };

    let instance_id = s.server_config.instance_id.clone().unwrap_or_default();
    let user_count = s.user_repo.count_by_tenant(tenant_id).await.unwrap_or(0);
    let backend_count = s.backend_repo.count_by_tenant(tenant_id).await.unwrap_or(0);
    let api_key_count = s.api_key_repo.count_by_tenant(tenant_id).await.unwrap_or(0);

    let payload = SyncPushPayload {
        instance_id,
        user_count,
        backend_count,
        api_key_count,
        total_requests_30d: 0,
        total_cost_30d: 0.0,
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: 0,
    };

    match svc.push(&platform_url, &api_key, payload).await {
        Ok(_) => {
            let now = chrono::Utc::now().to_rfc3339();
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok", "synced_at": now }))).into_response()
        }
        Err(e) => {
            tracing::error!("Sync push failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn pull(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;
    let svc = PlatformSyncService::new(s.setting_repo.clone(), tenant_id);

    let state = match svc.get_state().await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    let (platform_url, api_key) = match &state {
        gateway_tenant::sync::SyncState::Linked { platform_url, .. } => {
            let map = s.setting_repo.get_map(tenant_id).await.unwrap_or_default();
            let key = map.get("sync_api_key").cloned().unwrap_or_default();
            (platform_url.clone(), key)
        }
        _ => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Not linked to platform"}))).into_response(),
    };

    match svc.pull(&platform_url, &api_key).await {
        Ok(resp) => {
            (StatusCode::OK, Json(serde_json::json!({
                "plan": resp.plan,
                "features_updated": resp.license_key.is_some(),
                "messages": resp.messages,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Sync pull failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn unlink(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let svc = PlatformSyncService::new(s.setting_repo.clone(), auth.0.tenant_id);

    match svc.unlink().await {
        Ok(_) => {
            (StatusCode::OK, Json(serde_json::json!({
                "status": "unlinked",
                "message": "Instance unlinked. Reverted to Community edition features."
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Sync unlink failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
