use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::backend::{BackendProviderType, CreateBackend};
use gateway_tenant::TenantContext;
use gateway_audit::events::{AuditEvent, EventType};

/// `GET /api/v1/backends`
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.backend_repo.list_active_by_tenant(tenant_id).await {
        Ok(backends) => {
            // Enrich with live health status from checker
            let enriched: Vec<serde_json::Value> = backends.iter().map(|b| {
                let live_health = s.health_checker.get_status(b.id);
                serde_json::json!({
                    "id": b.id,
                    "name": b.name,
                    "provider_type": b.provider_type,
                    "endpoint": b.endpoint,
                    "health_status": live_health,
                    "priority": b.priority,
                    "weight": b.weight,
                    "timeout_ms": b.timeout_ms,
                    "max_retries": b.max_retries,
                    "is_active": b.is_active,
                    "last_health_check": b.last_health_check,
                    "active_connections": s.gateway_engine.load_balancer.active_count(b.id),
                    "created_at": b.created_at,
                    "updated_at": b.updated_at,
                })
            }).collect();
            let total = enriched.len();
            (StatusCode::OK, Json(serde_json::json!({ "backends": enriched, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list backends: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list backends"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBackendRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub provider_type: BackendProviderType,
    #[validate(url)]
    pub endpoint: String,
    /// API key or credentials (will be encrypted at rest)
    pub credentials: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default = "default_weight")]
    #[validate(range(min = 1))]
    pub weight: i32,
    #[serde(default = "default_timeout")]
    #[validate(range(min = 100, max = 300000))]
    pub timeout_ms: i32,
    #[serde(default = "default_retries")]
    #[validate(range(min = 0, max = 10))]
    pub max_retries: i32,
}
fn default_priority() -> i32 { 0 }
fn default_weight() -> i32 { 1 }
fn default_timeout() -> i32 { 30000 }
fn default_retries() -> i32 { 3 }

/// `POST /api/v1/backends`
pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(body): Json<CreateBackendRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;

    // Quota check
    if let Some(Extension(ctx)) = &tenant_ctx {
        if let Ok(count) = s.backend_repo.count_by_tenant(tenant_id).await {
            if !ctx.can_add_backend(count as i32) {
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                    "error": "Backend quota exceeded for this tenant"
                }))).into_response();
            }
        }
    }

    // Encrypt credentials if provided
    let encrypted_creds = body.credentials.as_deref().map(|c| {
        // Use field encryptor if available, otherwise store as-is (dev mode)
        if let Some(ref key) = s.server_config.encryption_key {
            gateway_core::FieldEncryptor::new(key)
                .ok()
                .and_then(|enc| enc.encrypt(c).ok())
                .unwrap_or_else(|| c.to_string())
        } else {
            c.to_string()
        }
    });

    let input = CreateBackend {
        tenant_id,
        name: body.name.clone(),
        provider_type: body.provider_type.clone(),
        endpoint: body.endpoint.clone(),
        encrypted_credentials: encrypted_creds,
        priority: body.priority,
        weight: body.weight,
        timeout_ms: body.timeout_ms,
        max_retries: body.max_retries,
    };

    match s.backend_repo.create(input).await {
        Ok(backend) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::BackendCreated, "backend")
                    .with_user(auth.0.user_id)
                    .with_resource_id(backend.id.to_string())
                    .with_details(serde_json::json!({
                        "name": body.name,
                        "provider_type": body.provider_type,
                        "endpoint": body.endpoint,
                    }))
            );

            (StatusCode::CREATED, Json(serde_json::json!({
                "id": backend.id,
                "name": backend.name,
                "provider_type": backend.provider_type,
                "endpoint": backend.endpoint,
                "priority": backend.priority,
                "weight": backend.weight,
                "is_active": backend.is_active,
                "created_at": backend.created_at,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create backend: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create backend"}))).into_response()
        }
    }
}

/// `GET /api/v1/backends/:id`
pub async fn get(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match s.backend_repo.find_by_id(id, auth.0.tenant_id).await {
        Ok(b) => {
            let live_health = s.health_checker.get_status(b.id);
            (StatusCode::OK, Json(serde_json::json!({
                "id": b.id,
                "name": b.name,
                "provider_type": b.provider_type,
                "endpoint": b.endpoint,
                "health_status": live_health,
                "priority": b.priority,
                "weight": b.weight,
                "timeout_ms": b.timeout_ms,
                "max_retries": b.max_retries,
                "is_active": b.is_active,
                "last_health_check": b.last_health_check,
                "active_connections": s.gateway_engine.load_balancer.active_count(b.id),
                "created_at": b.created_at,
                "updated_at": b.updated_at,
            }))).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Backend not found"}))).into_response(),
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBackendRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    #[validate(url)]
    pub endpoint: Option<String>,
    pub credentials: Option<String>,
    pub priority: Option<i32>,
    #[validate(range(min = 1))]
    pub weight: Option<i32>,
    #[validate(range(min = 100, max = 300000))]
    pub timeout_ms: Option<i32>,
    pub is_active: Option<bool>,
}

/// `PUT /api/v1/backends/:id`
pub async fn update(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateBackendRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;

    // Verify backend exists and belongs to tenant
    let _existing = match s.backend_repo.find_by_id(id, tenant_id).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Backend not found"}))).into_response(),
    };

    // Build dynamic update SQL — for now use individual field updates
    // TODO: add a proper update method to BackendRepository
    if let Some(false) = body.is_active {
        if let Err(e) = s.backend_repo.delete(id, tenant_id).await {
            tracing::error!("Failed to deactivate backend: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to update backend"}))).into_response();
        }
    }

    s.audit_service.log(
        AuditEvent::new(tenant_id, EventType::BackendUpdated, "backend")
            .with_user(auth.0.user_id)
            .with_resource_id(id.to_string())
    );

    // Return current state
    match s.backend_repo.find_by_id(id, tenant_id).await {
        Ok(b) => (StatusCode::OK, Json(serde_json::json!(b))).into_response(),
        Err(_) => (StatusCode::OK, Json(serde_json::json!({"id": id, "updated": true}))).into_response(),
    }
}

/// `DELETE /api/v1/backends/:id`
pub async fn delete(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.backend_repo.delete(id, tenant_id).await {
        Ok(_) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::BackendDeleted, "backend")
                    .with_user(auth.0.user_id)
                    .with_resource_id(id.to_string())
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to delete backend: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete backend"}))).into_response()
        }
    }
}
