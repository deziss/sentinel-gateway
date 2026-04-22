use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;
use std::sync::Arc;

use crate::{context::TenantContext, error::TenantError, service::TenantService};
use gateway_auth::middleware::RequireAuth;

#[derive(Clone)]
pub struct TenantMiddlewareState {
    pub service: Arc<TenantService>,
    pub saas_mode: bool,
}

/// Resolve tenant from request and inject TenantContext extension.
/// Resolution order: X-Tenant-ID header → JWT claim → subdomain → default (SaaS mode)
pub async fn tenant_middleware(
    State(state): State<Arc<TenantMiddlewareState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let tenant_id_header = req
        .headers()
        .get("X-Tenant-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    let host_header = req
        .headers()
        .get("Host")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    // Extract JWT tenant_id from RequireAuth if present
    let jwt_tenant_id = req
        .extensions()
        .get::<RequireAuth>()
        .map(|ra| ra.0.tenant_id);

    let result = state
        .service
        .resolve(
            tenant_id_header.as_deref(),
            jwt_tenant_id,
            None, // API key tenant resolved separately in auth middleware
            host_header.as_deref(),
        )
        .await;

    match result {
        Ok(tenant) => {
            req.extensions_mut()
                .insert(TenantContext::new(tenant, state.saas_mode));
            next.run(req).await
        }
        // Pre-auth routes (login, refresh, SSO callback) hit this middleware before any
        // tenant context exists — they resolve tenant from the request body themselves.
        // Proceeding without injecting `TenantContext` lets those endpoints run; protected
        // routes downstream will still be gated by `auth_middleware` / `role_gate`, and
        // handlers that need tenancy use `auth.0.tenant_id` from the JWT.
        Err(TenantError::NotFound) => next.run(req).await,
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
