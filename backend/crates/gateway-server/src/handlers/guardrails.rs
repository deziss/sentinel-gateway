//! Guardrails HTTP handler — CRUD for tenant guardrail rules.
//!
//! Stored rules are loaded into the `GuardrailPipeline` at request time.
//! A cache could be added in `AppState` if DB lookups become a bottleneck —
//! currently each LLM request hits `list_active()` (indexed lookup by tenant).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_audit::events::{AuditEvent, EventType};
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::guardrail_rule::{CreateGuardrailRule, UpdateGuardrailRule};

const VALID_KINDS: &[&str] = &["regex", "length", "json_schema", "pii"];
const VALID_STAGES: &[&str] = &["pre_call", "post_call", "logging_only"];
const VALID_MODES: &[&str] = &["block", "redact", "flag"];

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRuleRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 1, max = 64))]
    pub kind: String,
    #[validate(length(min = 1, max = 32))]
    pub stage: String,
    #[validate(length(min = 1, max = 32))]
    pub mode: String,
    #[serde(default = "default_category")]
    #[validate(length(min = 1, max = 64))]
    pub category: String,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default = "default_priority")]
    #[validate(range(min = 0, max = 10_000))]
    pub priority: i32,
}

fn default_category() -> String { "general".to_string() }
fn default_priority() -> i32 { 100 }

/// `POST /api/v1/guardrails` — create a new rule.
pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    // Validate enum-like string fields at the handler level (DB is permissive).
    if !VALID_KINDS.contains(&body.kind.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": format!("invalid kind: {} (allowed: {:?})", body.kind, VALID_KINDS)
        }))).into_response();
    }
    if !VALID_STAGES.contains(&body.stage.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": format!("invalid stage: {} (allowed: {:?})", body.stage, VALID_STAGES)
        }))).into_response();
    }
    if !VALID_MODES.contains(&body.mode.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": format!("invalid mode: {} (allowed: {:?})", body.mode, VALID_MODES)
        }))).into_response();
    }

    let input = CreateGuardrailRule {
        tenant_id: auth.0.tenant_id,
        name: body.name.clone(),
        kind: body.kind,
        stage: body.stage,
        mode: body.mode,
        category: body.category,
        config: body.config,
        priority: body.priority,
        created_by: Some(auth.0.user_id),
    };

    match s.guardrail_rule_repo.create(input).await {
        Ok(rule) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "guardrail")
                    .with_user(auth.0.user_id)
                    .with_resource_id(rule.id.to_string())
                    .with_details(serde_json::json!({
                        "action": "guardrail.create",
                        "name": body.name,
                        "kind": rule.kind,
                    })),
            );
            (StatusCode::CREATED, Json(serde_json::json!(rule))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create guardrail rule");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create rule"}))).into_response()
        }
    }
}

/// `GET /api/v1/guardrails` — list all rules for the tenant.
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    match s.guardrail_rule_repo.list_by_tenant(auth.0.tenant_id).await {
        Ok(rules) => {
            let total = rules.len();
            (StatusCode::OK, Json(serde_json::json!({ "rules": rules, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list guardrail rules");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list rules"}))).into_response()
        }
    }
}

/// `GET /api/v1/guardrails/:id` — get a rule by id.
pub async fn get(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match s.guardrail_rule_repo.get(auth.0.tenant_id, id).await {
        Ok(Some(rule)) => (StatusCode::OK, Json(serde_json::json!(rule))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get rule");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateRuleRequest {
    pub kind: Option<String>,
    pub stage: Option<String>,
    pub mode: Option<String>,
    pub category: Option<String>,
    pub config: Option<serde_json::Value>,
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
}

/// `PUT /api/v1/guardrails/:id` — update fields.
pub async fn update(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRuleRequest>,
) -> impl IntoResponse {
    // Validate enum-like fields if provided
    if let Some(ref k) = body.kind {
        if !VALID_KINDS.contains(&k.as_str()) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("invalid kind: {k}")
            }))).into_response();
        }
    }
    if let Some(ref stage) = body.stage {
        if !VALID_STAGES.contains(&stage.as_str()) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("invalid stage: {stage}")
            }))).into_response();
        }
    }
    if let Some(ref mode) = body.mode {
        if !VALID_MODES.contains(&mode.as_str()) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("invalid mode: {mode}")
            }))).into_response();
        }
    }

    let update = UpdateGuardrailRule {
        kind: body.kind,
        stage: body.stage,
        mode: body.mode,
        category: body.category,
        config: body.config,
        priority: body.priority,
        is_active: body.is_active,
    };

    match s.guardrail_rule_repo.update(auth.0.tenant_id, id, update).await {
        Ok(rule) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "guardrail")
                    .with_user(auth.0.user_id)
                    .with_resource_id(id.to_string())
                    .with_details(serde_json::json!({"action": "guardrail.update"})),
            );
            (StatusCode::OK, Json(serde_json::json!(rule))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update rule");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to update"}))).into_response()
        }
    }
}

/// `DELETE /api/v1/guardrails/:id` — delete a rule.
pub async fn delete(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match s.guardrail_rule_repo.delete(auth.0.tenant_id, id).await {
        Ok(_) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "guardrail")
                    .with_user(auth.0.user_id)
                    .with_resource_id(id.to_string())
                    .with_details(serde_json::json!({"action": "guardrail.delete"})),
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete rule");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TestRequest {
    pub content: String,
}

/// `POST /api/v1/guardrails/test` — test the configured pipeline against arbitrary content.
/// Useful for verifying rules before enabling them on live traffic.
pub async fn test_pipeline(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<TestRequest>,
) -> impl IntoResponse {
    let rules = match s.guardrail_rule_repo.list_active(auth.0.tenant_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "Failed to load rules");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to load rules"}))).into_response();
        }
    };

    let pipeline = crate::guardrails_build::build_pipeline(&rules);
    let pre = pipeline.run(
        gateway_policy::GuardrailStage::PreCall,
        &body.content,
        None,
        Some(auth.0.tenant_id),
        Some(auth.0.user_id),
    ).await;

    let (final_content, results) = pre;

    let summary: Vec<serde_json::Value> = results.iter().map(|r| serde_json::json!({
        "name": r.name,
        "outcome": match &r.outcome {
            gateway_policy::GuardrailOutcome::Pass => "pass".to_string(),
            gateway_policy::GuardrailOutcome::Modify { .. } => "modify".to_string(),
            gateway_policy::GuardrailOutcome::Block { reason, category } =>
                format!("block({category}): {reason}"),
            gateway_policy::GuardrailOutcome::Flag { reason, category } =>
                format!("flag({category}): {reason}"),
        },
        "duration_ms": r.duration_ms,
    })).collect();

    (StatusCode::OK, Json(serde_json::json!({
        "input": body.content,
        "final_content": final_content,
        "results": summary,
        "blocked": results.iter().any(|r| r.is_blocked()),
    }))).into_response()
}
