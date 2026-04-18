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
        Err(TenantError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Tenant not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
