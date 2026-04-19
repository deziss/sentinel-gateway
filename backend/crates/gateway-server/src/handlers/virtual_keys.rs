use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::virtual_key::CreateVirtualKey;
use gateway_audit::events::{AuditEvent, EventType};

/// `GET /api/v1/virtual-keys`
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    match s.virtual_key_repo.list_by_tenant(auth.0.tenant_id).await {
        Ok(keys) => {
            // Never return key_hash; use key_prefix for UI display.
            let total = keys.len();
            (StatusCode::OK, Json(serde_json::json!({"virtual_keys": keys, "total": total}))).into_response()
        }
        Err(e) => {
            tracing::error!("List virtual keys failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateVirtualKeyRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub backend_id: Uuid,
    pub team_id: Option<Uuid>,
    pub allowed_models: Option<Vec<String>>,
    #[validate(range(min = 1))]
    pub rate_limit_rpm: Option<i32>,
    #[validate(range(min = 1))]
    pub token_limit_tpm: Option<i32>,
    #[validate(range(min = 0.0))]
    pub budget_daily: Option<f64>,
    #[validate(range(min = 0.0))]
    pub budget_monthly: Option<f64>,
    pub metadata: Option<serde_json::Value>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// `POST /api/v1/virtual-keys`
///
/// Generates `vk_<base64>` plaintext; stores SHA-256 hash. Plaintext is
/// returned **once** in the response and cannot be recovered.
pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateVirtualKeyRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    // Verify backend belongs to tenant
    if s.backend_repo.find_by_id(body.backend_id, auth.0.tenant_id).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Backend not found"}))).into_response();
    }

    // Generate key: "vk_" + 32 random bytes base64-url
    let (plaintext, hash, prefix) = generate_virtual_key();

    let input = CreateVirtualKey {
        tenant_id: auth.0.tenant_id,
        team_id: body.team_id,
        user_id: Some(auth.0.user_id),
        name: body.name.clone(),
        key_hash: hash,
        key_prefix: prefix,
        backend_id: body.backend_id,
        allowed_models: body.allowed_models,
        rate_limit_rpm: body.rate_limit_rpm,
        token_limit_tpm: body.token_limit_tpm,
        budget_daily: body.budget_daily,
        budget_monthly: body.budget_monthly,
        metadata: body.metadata.unwrap_or(serde_json::json!({})),
        expires_at: body.expires_at,
    };

    match s.virtual_key_repo.create(input).await {
        Ok(vk) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::ApiKeyCreated, "virtual_key")
                    .with_user(auth.0.user_id)
                    .with_resource_id(vk.id.to_string())
                    .with_details(serde_json::json!({"name": body.name, "backend_id": body.backend_id}))
            );
            (StatusCode::CREATED, Json(serde_json::json!({
                "id": vk.id,
                "name": vk.name,
                "key": plaintext,           // shown once
                "key_prefix": vk.key_prefix,
                "backend_id": vk.backend_id,
                "team_id": vk.team_id,
                "allowed_models": vk.allowed_models,
                "rate_limit_rpm": vk.rate_limit_rpm,
                "token_limit_tpm": vk.token_limit_tpm,
                "budget_daily": vk.budget_daily,
                "budget_monthly": vk.budget_monthly,
                "expires_at": vk.expires_at,
                "created_at": vk.created_at,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Create virtual key failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create"}))).into_response()
        }
    }
}

/// `DELETE /api/v1/virtual-keys/:id`
pub async fn revoke(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match s.virtual_key_repo.revoke(id, auth.0.tenant_id).await {
        Ok(_) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::ApiKeyRevoked, "virtual_key")
                    .with_user(auth.0.user_id)
                    .with_resource_id(id.to_string())
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("Revoke virtual key failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to revoke"}))).into_response()
        }
    }
}

fn generate_virtual_key() -> (String, String, String) {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let mut rng = rand::thread_rng();
    let raw: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    let plaintext = format!("vk_{}", URL_SAFE_NO_PAD.encode(&raw));

    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    let hash = hex::encode(hasher.finalize());

    let prefix = plaintext.chars().take(12).collect::<String>();
    (plaintext, hash, prefix)
}
