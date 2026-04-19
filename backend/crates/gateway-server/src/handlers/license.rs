use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, Extension};
use std::sync::Arc;

use crate::handlers::feature_gate::require_super_admin;
use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;

/// `GET /api/v1/license/status` — current license activation state.
pub async fn status(
    State(s): State<Arc<AppState>>,
    Extension(_auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let state = s.activation_service.state().await;
    let features = s.activation_service.features().await;

    (StatusCode::OK, Json(serde_json::json!({
        "activation": state,
        "features": features,
        "deployment_mode": s.deployment_mode,
        "instance_id": s.server_config.instance_id,
    }))).into_response()
}

/// `GET /api/v1/license/features` — publicly-visible feature matrix for the
/// current plan. Frontend uses this to drive upsell UI + conditional nav.
pub async fn features(
    State(s): State<Arc<AppState>>,
    Extension(_auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let flags = s.activation_service.features().await;
    (StatusCode::OK, Json(serde_json::json!({
        "plan": flags.plan,
        "features": flags,
    }))).into_response()
}

/// `POST /api/v1/license/activate` — manually trigger license activation.
pub async fn activate(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    match s.activation_service.activate().await {
        Ok(features) => {
            let state = s.activation_service.state().await;
            (StatusCode::OK, Json(serde_json::json!({
                "activation": state,
                "features": features,
                "message": "License activation completed",
            }))).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": e.to_string(),
            }))).into_response()
        }
    }
}
