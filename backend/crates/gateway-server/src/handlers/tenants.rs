use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::handlers::feature_gate::require_super_admin;
use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::tenant::{CreateTenant, UpdateTenant};
use gateway_license::DeploymentMode;
use gateway_audit::events::{AuditEvent, EventType};

pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    match s.tenant_repo.list().await {
        Ok(tenants) => {
            let total = tenants.len();
            (StatusCode::OK, Json(serde_json::json!({ "tenants": tenants, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list tenants: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list tenants"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTenantRequest {
    #[validate(length(min = 1, max = 255, message = "Name must be 1-255 characters"))]
    pub name: String,
    #[validate(length(min = 1, max = 63, message = "Slug must be 1-63 characters"))]
    pub slug: String,
    #[serde(default = "default_plan")]
    #[validate(length(min = 1, max = 50))]
    pub plan: String,
    #[serde(default = "default_max_users")]
    #[validate(range(min = 1, message = "Must be at least 1"))]
    pub max_users: i32,
    #[serde(default = "default_max_api_keys")]
    #[validate(range(min = 1, message = "Must be at least 1"))]
    pub max_api_keys: i32,
    #[serde(default = "default_max_backends")]
    #[validate(range(min = 1, message = "Must be at least 1"))]
    pub max_backends: i32,
}
fn default_plan() -> String { "community".to_string() }
fn default_max_users() -> i32 { 5 }
fn default_max_api_keys() -> i32 { 10 }
fn default_max_backends() -> i32 { 3 }

pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    if s.deployment_mode == DeploymentMode::Local {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "Multi-tenant not available in Community edition"
        }))).into_response();
    }

    if !s.features.multi_tenant {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "Multi-tenant requires Professional or Enterprise plan"
        }))).into_response();
    }

    let input = CreateTenant {
        name: body.name.clone(),
        slug: body.slug.clone(),
        plan: body.plan,
        max_users: body.max_users,
        max_api_keys: body.max_api_keys,
        max_backends: body.max_backends,
    };

    match s.tenant_repo.create(input).await {
        Ok(tenant) => {
            s.audit_service.log(
                AuditEvent::new(tenant.id, EventType::TenantCreated, "tenant")
                    .with_user(auth.0.user_id)
                    .with_resource_id(tenant.id.to_string())
                    .with_details(serde_json::json!({"name": body.name, "slug": body.slug}))
            );
            (StatusCode::CREATED, Json(serde_json::json!(tenant))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create tenant: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create tenant"}))).into_response()
        }
    }
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !auth.0.role.is_super_admin() && auth.0.tenant_id != id {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Tenant not found"}))).into_response();
    }
    match s.tenant_repo.find_by_id(id).await {
        Ok(tenant) => (StatusCode::OK, Json(serde_json::json!(tenant))).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Tenant not found"}))).into_response(),
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTenantRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub plan: Option<String>,
    #[validate(range(min = 1))]
    pub max_users: Option<i32>,
    #[validate(range(min = 1))]
    pub max_api_keys: Option<i32>,
    #[validate(range(min = 1))]
    pub max_backends: Option<i32>,
    pub is_active: Option<bool>,
    pub settings: Option<serde_json::Value>,
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateTenantRequest>,
) -> impl IntoResponse {
    // TenantAdmin may update own tenant; SuperAdmin may update any.
    if !auth.0.role.is_super_admin() && auth.0.tenant_id != id {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "Cannot modify another tenant"
        }))).into_response();
    }
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let input = UpdateTenant {
        name: body.name,
        plan: body.plan,
        settings: body.settings,
        max_users: body.max_users,
        max_api_keys: body.max_api_keys,
        max_backends: body.max_backends,
        is_active: body.is_active,
        license_key: None,
    };

    match s.tenant_repo.update(id, input).await {
        Ok(tenant) => {
            s.tenant_service.invalidate_cache(id);
            (StatusCode::OK, Json(serde_json::json!(tenant))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update tenant: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to update tenant"}))).into_response()
        }
    }
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    match s.tenant_repo.delete(id).await {
        Ok(_) => {
            s.tenant_service.invalidate_cache(id);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to delete tenant: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete tenant"}))).into_response()
        }
    }
}
