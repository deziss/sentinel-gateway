use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, Extension};
use std::sync::Arc;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
#[allow(unused_imports)] // DeploymentMode only matched in `saas` cfg branches; FeatureFlags/Plan only in `!saas`.
use gateway_license::{DeploymentMode, FeatureFlags, Plan};

/// `GET /api/v1/license/status` — current license activation state.
pub async fn status(
    State(s): State<Arc<AppState>>,
    #[cfg_attr(not(feature = "saas"), allow(unused_variables))]
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let mode = s.deployment_mode.clone();
    
    match mode {
        #[cfg(feature = "saas")]
        DeploymentMode::Platform => {
            // SaaS: Return per-tenant status
            let flags = s.tenant_license_service.resolve(auth.0.tenant_id).await;
            let license = s.tenant_license_repo.find_by_tenant_id(auth.0.tenant_id).await.ok().flatten();
            
            (StatusCode::OK, Json(serde_json::json!({
                "mode": "platform",
                "tenant_id": auth.0.tenant_id,
                "plan": flags.plan,
                "features": flags,
                "license_status": license.as_ref().map(|l| l.status.clone()).unwrap_or_else(|| "none".to_string()),
                "expires_at": license.and_then(|l| l.expires_at),
            }))).into_response()
        }
        #[cfg(feature = "saas")]
        _ => {
            // PaaS/Local with SaaS binary
            let state = s.activation_service.state().await;
            let features = s.activation_service.features().await;
            (StatusCode::OK, Json(serde_json::json!({
                "mode": format!("{:?}", mode).to_lowercase(),
                "activation": state,
                "features": features,
                "instance_id": s.server_config.instance_id,
            }))).into_response()
        }
        #[cfg(not(feature = "saas"))]
        _ => {
            // Community build
            (StatusCode::OK, Json(serde_json::json!({
                "mode": "community",
                "plan": "community",
                "features": FeatureFlags::for_plan(Plan::Community),
            }))).into_response()
        }
    }
}

/// `GET /api/v1/license/features` — publicly-visible feature matrix.
pub async fn features(
    State(s): State<Arc<AppState>>,
    #[cfg_attr(not(feature = "saas"), allow(unused_variables))]
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let flags = match s.deployment_mode {
        #[cfg(feature = "saas")]
        DeploymentMode::Platform => s.tenant_license_service.resolve(auth.0.tenant_id).await,
        #[cfg(feature = "saas")]
        _ => s.activation_service.features().await,
        #[cfg(not(feature = "saas"))]
        _ => FeatureFlags::for_plan(Plan::Community),
    };

    (StatusCode::OK, Json(serde_json::json!({
        "plan": flags.plan,
        "features": flags,
    }))).into_response()
}

/// `POST /api/v1/license/activate` — manually trigger license activation/refresh.
pub async fn activate(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    // Requires SuperAdmin for Instance or TenantAdmin for Tenant activation
    if !auth.0.role.is_super_admin() && !auth.0.role.is_at_least_tenant_admin() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": "Insufficient permissions" }))).into_response();
    }

    match s.deployment_mode {
        #[cfg(feature = "saas")]
        DeploymentMode::Platform => {
            match s.tenant_license_service.refresh(auth.0.tenant_id).await {
                Ok(f) => (StatusCode::OK, Json(serde_json::json!({ "message": "Tenant license refreshed", "features": f }))).into_response(),
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
            }
        }
        #[cfg(feature = "saas")]
        _ => {
            if !auth.0.role.is_super_admin() {
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": "SuperAdmin required" }))).into_response();
            }
            match s.activation_service.activate().await {
                Ok(f) => (StatusCode::OK, Json(serde_json::json!({ "message": "Instance activated", "features": f }))).into_response(),
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
            }
        }
        #[cfg(not(feature = "saas"))]
        _ => (StatusCode::NOT_IMPLEMENTED, Json(serde_json::json!({ "error": "Licensing not enabled in this build" }))).into_response(),
    }
}
