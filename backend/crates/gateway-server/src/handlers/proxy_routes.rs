use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::route::{CreateRoute, RouteProtocol};
use gateway_audit::events::{AuditEvent, EventType};

/// `GET /api/v1/routes`
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.route_repo.list_active_by_tenant(tenant_id).await {
        Ok(routes) => {
            let total = routes.len();
            (StatusCode::OK, Json(serde_json::json!({ "routes": routes, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list routes: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list routes"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRouteRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub protocol: RouteProtocol,
    #[validate(length(min = 1, max = 512))]
    pub path_pattern: String,
    pub backend_id: Uuid,
    #[serde(default)]
    pub strip_prefix: bool,
    #[serde(default = "default_rewrite_rules")]
    pub rewrite_rules: serde_json::Value,
}
fn default_rewrite_rules() -> serde_json::Value { serde_json::json!({}) }

/// `POST /api/v1/routes`
pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateRouteRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;

    // Validate backend exists and belongs to tenant
    if s.backend_repo.find_by_id(body.backend_id, tenant_id).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Backend not found or does not belong to this tenant"
        }))).into_response();
    }

    let input = CreateRoute {
        tenant_id,
        name: body.name.clone(),
        protocol: body.protocol.clone(),
        path_pattern: body.path_pattern.clone(),
        backend_id: body.backend_id,
        strip_prefix: body.strip_prefix,
        rewrite_rules: body.rewrite_rules,
    };

    match s.route_repo.create(input).await {
        Ok(route) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::RouteCreated, "route")
                    .with_user(auth.0.user_id)
                    .with_resource_id(route.id.to_string())
                    .with_details(serde_json::json!({
                        "name": body.name,
                        "protocol": body.protocol,
                        "path_pattern": body.path_pattern,
                        "backend_id": body.backend_id,
                    }))
            );
            (StatusCode::CREATED, Json(serde_json::json!(route))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create route: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create route"}))).into_response()
        }
    }
}

/// `DELETE /api/v1/routes/:id`
pub async fn delete(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.route_repo.delete(id, tenant_id).await {
        Ok(_) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::RouteDeleted, "route")
                    .with_user(auth.0.user_id)
                    .with_resource_id(id.to_string())
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to delete route: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete route"}))).into_response()
        }
    }
}
