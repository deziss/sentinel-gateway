use axum::{Json, extract::{State, Path}, http::{HeaderMap, StatusCode}, response::IntoResponse, Extension};
use axum::extract::ConnectInfo;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::api_key;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::api_key::CreateApiKey;
use gateway_audit::events::{AuditEvent, EventType};

fn extract_client_ip(headers: &HeaderMap, addr: Option<&ConnectInfo<SocketAddr>>) -> String {
    headers.get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| addr.map(|a| a.0.ip().to_string()).unwrap_or_else(|| "unknown".to_string()))
}

pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;
    match s.api_key_repo.list_by_tenant(tenant_id).await {
        Ok(keys) => (StatusCode::OK, Json(serde_json::json!({"api_keys": keys}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch API keys: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to fetch API keys"}))).into_response()
        }
    }
}

#[derive(Deserialize, Validate)]
pub struct CreateKeyRequest {
    #[validate(length(min = 1, max = 255, message = "Name must be 1-255 characters"))]
    pub name: String,
    #[validate(length(min = 1, message = "At least one scope is required"))]
    pub scopes: Vec<String>,
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    tenant_ctx: Option<Extension<gateway_tenant::TenantContext>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(body): Json<CreateKeyRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;
    let user_id = auth.0.user_id;
    let client_ip = extract_client_ip(&headers, connect_info.as_ref());

    // Quota check
    if let Some(Extension(ctx)) = &tenant_ctx {
        if let Ok(count) = s.api_key_repo.count_by_tenant(tenant_id).await {
            if !ctx.can_add_api_key(count as i32) {
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                    "error": "API key quota exceeded for this tenant"
                }))).into_response();
            }
        }
    }

    let (plaintext, hash) = api_key::generate_api_key();

    let input = CreateApiKey {
        tenant_id,
        user_id,
        key_hash: hash,
        name: body.name.clone(),
        scopes: body.scopes,
        rate_limit_rpm: None,
        budget_daily: None,
        budget_monthly: None,
        expires_at: None,
    };

    match s.api_key_repo.create(input).await {
        Ok(key) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::ApiKeyCreated, "api_key")
                    .with_user(user_id)
                    .with_resource_id(key.id.to_string())
                    .with_details(serde_json::json!({"name": body.name}))
                    .with_ip(&client_ip)
            );

            (StatusCode::CREATED, Json(serde_json::json!({"key": plaintext, "metadata": key}))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create API key"}))).into_response()
        }
    }
}

pub async fn revoke(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;
    let user_id = auth.0.user_id;
    let client_ip = extract_client_ip(&headers, connect_info.as_ref());

    match s.api_key_repo.revoke(id, tenant_id).await {
        Ok(_) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::ApiKeyRevoked, "api_key")
                    .with_user(user_id)
                    .with_resource_id(id.to_string())
                    .with_details(serde_json::json!({"revoked": true}))
                    .with_ip(&client_ip)
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to revoke API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to revoke API key"}))).into_response()
        }
    }
}
