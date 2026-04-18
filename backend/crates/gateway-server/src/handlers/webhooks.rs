use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::webhook_endpoint::CreateWebhookEndpoint;
use gateway_audit::events::{AuditEvent, EventType};

pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.webhook_repo.list_by_tenant(tenant_id).await {
        Ok(endpoints) => {
            let safe: Vec<serde_json::Value> = endpoints.iter().map(|e| serde_json::json!({
                "id": e.id,
                "url": e.url,
                "events": e.events,
                "is_active": e.is_active,
                "last_sent_at": e.last_sent_at,
                "created_at": e.created_at,
            })).collect();
            let total = safe.len();
            (StatusCode::OK, Json(serde_json::json!({ "webhooks": safe, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list webhooks: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list webhooks"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateWebhookRequest {
    #[validate(url(message = "Must be a valid URL"))]
    pub url: String,
    #[validate(length(min = 1, message = "At least one event type is required"))]
    pub events: Vec<String>,
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateWebhookRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let tenant_id = auth.0.tenant_id;
    let secret = generate_secret();

    let input = CreateWebhookEndpoint {
        tenant_id,
        url: body.url.clone(),
        events: body.events,
        secret: secret.clone(),
    };

    match s.webhook_repo.create(input).await {
        Ok(wh) => {
            s.audit_service.log(
                AuditEvent::new(tenant_id, EventType::WebhookCreated, "webhook")
                    .with_user(auth.0.user_id)
                    .with_resource_id(wh.id.to_string())
                    .with_details(serde_json::json!({"url": body.url}))
            );

            (StatusCode::CREATED, Json(serde_json::json!({
                "id": wh.id,
                "url": wh.url,
                "events": wh.events,
                "secret": secret,
                "is_active": wh.is_active,
                "created_at": wh.created_at,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create webhook: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create webhook"}))).into_response()
        }
    }
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.webhook_repo.delete(id, tenant_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to delete webhook: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete webhook"}))).into_response()
        }
    }
}

pub async fn test(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    let endpoint = match s.webhook_repo.find_by_id(id, tenant_id).await {
        Ok(e) => e,
        Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Webhook not found"}))).into_response(),
    };

    let test_event = AuditEvent::new(tenant_id, EventType::WebhookCreated, "test")
        .with_user(auth.0.user_id)
        .with_details(serde_json::json!({"test": true, "message": "This is a test webhook delivery"}));

    let dispatcher = gateway_audit::WebhookDispatcher::new(1);
    dispatcher.dispatch(&test_event, &[endpoint]).await;

    (StatusCode::OK, Json(serde_json::json!({"status": "sent", "message": "Test event dispatched"}))).into_response()
}

/// `GET /api/v1/webhooks/failures` — list recent webhook delivery failures (DLQ)
pub async fn list_failures(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.webhook_failure_repo.list_by_tenant(tenant_id, 100).await {
        Ok(failures) => {
            (StatusCode::OK, Json(serde_json::json!({
                "failures": failures,
                "total": failures.len(),
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list webhook failures: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list failures"}))).into_response()
        }
    }
}

/// `POST /api/v1/webhooks/failures/:id/retry` — requeue a failed webhook for immediate retry
pub async fn retry_failure(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = auth.0.tenant_id;

    match s.webhook_failure_repo.requeue(id, tenant_id).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "requeued"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to requeue webhook: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to requeue"}))).into_response()
        }
    }
}

fn generate_secret() -> String {
    use rand::Rng;
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let mut rng = rand::thread_rng();
    let raw: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    format!("whsec_{}", URL_SAFE_NO_PAD.encode(&raw))
}
