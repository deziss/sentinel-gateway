use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_auth::password::PasswordService;
use gateway_db::models::user::{CreateUser, UserRole, UserStatus};
use gateway_tenant::TenantContext;
use gateway_audit::events::{AuditEvent, EventType};

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub last_login_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;
    let caller_is_super_admin = auth.0.role.is_super_admin();

    match s.user_repo.list_by_tenant(tenant_id).await {
        Ok(users) => {
            let safe_users: Vec<UserResponse> = users.into_iter()
                .filter(|u| caller_is_super_admin || !matches!(u.role, UserRole::SuperAdmin))
                .map(|u| UserResponse {
                    id: u.id,
                    tenant_id: u.tenant_id,
                    email: u.email,
                    role: u.role,
                    status: u.status,
                    last_login_at: u.last_login_at,
                    created_at: u.created_at,
                    updated_at: u.updated_at,
                }).collect();
            let total = safe_users.len();
            (StatusCode::OK, Json(serde_json::json!({ "users": safe_users, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list users: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list users"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct InviteUserRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "Password must be 8-128 characters"))]
    pub password: String,
    #[serde(default = "default_role")]
    #[validate(length(min = 1, max = 50))]
    pub role: String,
}
fn default_role() -> String { "user".to_string() }

pub async fn invite(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(body): Json<InviteUserRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;

    // Quota check
    if let Some(Extension(ctx)) = &tenant_ctx {
        match s.user_repo.count_by_tenant(tenant_id).await {
            Ok(count) => {
                if !ctx.can_add_user(count as i32) {
                    return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                        "error": "User quota exceeded for this tenant"
                    }))).into_response();
                }
            }
            Err(e) => {
                tracing::error!("Failed to count users: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response();
            }
        }
    }

    let role = match body.role.to_lowercase().as_str() {
        "super_admin" => {
            if !auth.0.role.is_super_admin() {
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                    "error": "Only SuperAdmin can create SuperAdmin users"
                }))).into_response();
            }
            UserRole::SuperAdmin
        }
        "tenant_admin" => {
            if !auth.0.role.is_at_least_tenant_admin() {
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                    "error": "Only TenantAdmin or higher can create TenantAdmin users"
                }))).into_response();
            }
            UserRole::TenantAdmin
        }
        "read_only" => UserRole::ReadOnly,
        _ => UserRole::User,
    };

    let password_hash = match PasswordService::hash(&body.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Password hash error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response();
        }
    };

    let input = CreateUser {
        tenant_id,
        email: body.email.clone(),
        password_hash,
        role,
    };

    match s.user_repo.create(input).await {
        Ok(user) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::UserInvited, "user")
                    .with_user(auth.0.user_id)
                    .with_resource_id(user.id.to_string())
                    .with_details(serde_json::json!({"email": body.email}))
            );

            let resp = UserResponse {
                id: user.id,
                tenant_id: user.tenant_id,
                email: user.email,
                role: user.role,
                status: user.status,
                last_login_at: user.last_login_at,
                created_at: user.created_at,
                updated_at: user.updated_at,
            };
            (StatusCode::CREATED, Json(serde_json::json!(resp))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create user: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create user"}))).into_response()
        }
    }
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.user_repo.find_by_id(id, tenant_id).await {
        Ok(user) => {
            if matches!(user.role, UserRole::SuperAdmin) && !auth.0.role.is_super_admin() {
                return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"}))).into_response();
            }
            let resp = UserResponse {
                id: user.id,
                tenant_id: user.tenant_id,
                email: user.email,
                role: user.role,
                status: user.status,
                last_login_at: user.last_login_at,
                created_at: user.created_at,
                updated_at: user.updated_at,
            };
            (StatusCode::OK, Json(serde_json::json!(resp))).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"}))).into_response(),
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 50))]
    pub role: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub status: Option<String>,
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUserRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;

    // Block non-SuperAdmins from touching a SuperAdmin account.
    if let Ok(target) = s.user_repo.find_by_id(id, tenant_id).await {
        if matches!(target.role, UserRole::SuperAdmin) && !auth.0.role.is_super_admin() {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"}))).into_response();
        }
    }

    if let Some(ref role_str) = body.role {
        let role = match role_str.to_lowercase().as_str() {
            "super_admin" => {
                if !auth.0.role.is_super_admin() {
                    return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                        "error": "Only SuperAdmin can assign SuperAdmin role"
                    }))).into_response();
                }
                UserRole::SuperAdmin
            }
            "tenant_admin" => UserRole::TenantAdmin,
            "read_only" => UserRole::ReadOnly,
            _ => UserRole::User,
        };
        if let Err(e) = s.user_repo.update_role(id, tenant_id, role).await {
            tracing::error!("Failed to update user role: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to update user"}))).into_response();
        }
    }

    if let Some(ref status_str) = body.status {
        let status = match status_str.to_lowercase().as_str() {
            "inactive" => UserStatus::Inactive,
            "locked" => UserStatus::Locked,
            "pending" => UserStatus::Pending,
            _ => UserStatus::Active,
        };
        if let Err(e) = s.user_repo.update_status(id, tenant_id, status).await {
            tracing::error!("Failed to update user status: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to update user"}))).into_response();
        }
    }

    match s.user_repo.find_by_id(id, tenant_id).await {
        Ok(user) => {
            let resp = UserResponse {
                id: user.id,
                tenant_id: user.tenant_id,
                email: user.email,
                role: user.role,
                status: user.status,
                last_login_at: user.last_login_at,
                created_at: user.created_at,
                updated_at: user.updated_at,
            };
            (StatusCode::OK, Json(serde_json::json!(resp))).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"}))).into_response(),
    }
}

pub async fn deactivate(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    if let Ok(target) = s.user_repo.find_by_id(id, tenant_id).await {
        if matches!(target.role, UserRole::SuperAdmin) && !auth.0.role.is_super_admin() {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"}))).into_response();
        }
    }

    match s.user_repo.update_status(id, tenant_id, UserStatus::Inactive).await {
        Ok(_) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::UserDeactivated, "user")
                    .with_user(auth.0.user_id)
                    .with_resource_id(id.to_string())
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to deactivate user: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to deactivate user"}))).into_response()
        }
    }
}
