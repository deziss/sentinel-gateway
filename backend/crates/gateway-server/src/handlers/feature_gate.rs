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
use gateway_auth::context::AuthContext;
use gateway_auth::middleware::RequireAuth;
#[allow(unused_imports)] // DeploymentMode only matched in `saas` cfg branches below.
use gateway_license::{DeploymentMode, Feature, FeatureFlags};

/// Returns `Err(403)` when the authenticated caller is not a SuperAdmin.
/// Use at the top of platform-level handlers (tenants, organizations, license, admin).
///
/// The `Err` variant is an axum `Response`, which is larger than clippy's default threshold;
/// boxing it would add ergonomic friction at every call site (`?` wouldn't deref through the
/// Box) without any real memory-safety or perf benefit for an early-return path.
#[allow(clippy::result_large_err)]
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
pub async fn require_feature(
    state: &Arc<AppState>,
    auth: &AuthContext,
    feature: Feature,
) -> Result<FeatureFlags, Response> {
    require_feature_for_tenant(state, Some(auth.tenant_id), feature).await
}

/// Returns `Err(Response)` with a 402 Payment Required when the feature is
/// gated off for a specific tenant (or global instance if tenant_id is None).
pub async fn require_feature_for_tenant(
    state: &Arc<AppState>,
    #[cfg_attr(not(feature = "saas"), allow(unused_variables))]
    tenant_id: Option<uuid::Uuid>,
    feature: Feature,
) -> Result<FeatureFlags, Response> {
    // 1. Resolve effective flags based on build features and deployment mode
    let flags = match state.deployment_mode {
        #[cfg(feature = "saas")]
        DeploymentMode::Platform => {
            // SaaS: Resolve per-tenant from the cache/DB
            if let Some(tid) = tenant_id {
                state.tenant_license_service.resolve(tid).await
            } else {
                // If no tenant context in Platform mode, default to Community (safe default)
                FeatureFlags::for_plan(gateway_license::Plan::Community)
            }
        }
        #[cfg(feature = "saas")]
        _ => {
            // SaaS Binary running in Local/PaaS mode: Use the global instance features
            state.activation_service.features().await
        }
        #[cfg(not(feature = "saas"))]
        _ => {
            // Community Binary: ALWAYS Community plan, no activation service exists
            FeatureFlags::for_plan(gateway_license::Plan::Community)
        }
    };

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
