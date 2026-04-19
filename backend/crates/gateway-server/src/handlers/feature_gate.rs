//! Feature-gate helpers used inside handlers.
//!
//! Call `require_feature(state, Feature::X).await?` at the top of a handler
//! to return a 402/403 when the tenant's plan doesn't include the feature.
//!
//! Why inside the handler and not as a middleware? Middleware-level gating
//! needs the feature list at route-definition time, but our `FeatureFlags`
//! live behind a runtime `ActivationService` (license refreshes every N hours).
//! Per-request check is the simpler contract and adds ~1 µs to request time.

use axum::{Json, http::StatusCode, response::Response, response::IntoResponse};
use std::sync::Arc;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_license::{Feature, FeatureFlags};

/// Returns `Err(403)` when the authenticated caller is not a SuperAdmin.
/// Use at the top of platform-level handlers (tenants, organizations, license, admin).
pub fn require_super_admin(auth: &RequireAuth) -> Result<(), Response> {
    if auth.0.role.is_super_admin() {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": {
                    "code": "forbidden",
                    "message": "SuperAdmin role required"
                }
            })),
        )
            .into_response())
    }
}

/// Returns `Err(Response)` with a 402 Payment Required when the feature is
/// gated off. Handlers propagate the early return via `?`.
pub async fn require_feature(state: &Arc<AppState>, feature: Feature) -> Result<FeatureFlags, Response> {
    let flags = state.activation_service.features().await;
    if feature.check(&flags) {
        Ok(flags)
    } else {
        Err(gated_response(feature, &flags))
    }
}

/// Build a machine-readable "feature gated" response. Frontends parse
/// `error.code == "feature_gated"` + `required_plan` to render upsell UI.
pub fn gated_response(feature: Feature, flags: &FeatureFlags) -> Response {
    let required = feature.min_plan();
    (
        StatusCode::PAYMENT_REQUIRED, // 402 — widely supported and semantic-appropriate
        Json(serde_json::json!({
            "error": {
                "code": "feature_gated",
                "message": format!(
                    "Feature '{}' requires plan '{}' or higher. Current plan: '{}'.",
                    feature.name(),
                    required.as_str(),
                    flags.plan.as_str(),
                ),
                "feature": feature.name(),
                "required_plan": required.as_str(),
                "current_plan": flags.plan.as_str(),
            }
        })),
    ).into_response()
}
