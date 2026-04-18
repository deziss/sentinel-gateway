//! Prompt Management & Versioning API.
//!
//! Clients can:
//! - Create / list / get / delete prompt versions
//! - Deploy specific versions to labels (prod, staging, canary, dev, feature-flags)
//! - Reference prompts from LLM requests by (name, optional label) instead of inline text
//!
//! The `resolve` endpoint supports gateway-side template rendering: the client
//! provides variables and the gateway returns the rendered content.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use std::sync::Arc;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_audit::events::{AuditEvent, EventType};
use gateway_db::models::prompt::CreatePrompt;

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePromptRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 1, max = 1_048_576))] // 1MB max prompt
    pub content: String,
    #[serde(default)]
    pub variables: serde_json::Value,
    #[serde(default)]
    pub model_prefs: serde_json::Value,
    #[validate(length(max = 255))]
    pub default_model: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// `POST /api/v1/prompts` — create new version (auto-increments).
pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreatePromptRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let input = CreatePrompt {
        tenant_id: auth.0.tenant_id,
        name: body.name.clone(),
        content: body.content,
        variables: body.variables,
        model_prefs: body.model_prefs,
        default_model: body.default_model,
        metadata: body.metadata,
        created_by: Some(auth.0.user_id),
    };

    match s.prompt_repo.create(input).await {
        Ok(prompt) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "prompt")
                    .with_user(auth.0.user_id)
                    .with_resource_id(prompt.id.to_string())
                    .with_details(serde_json::json!({
                        "action": "prompt.create",
                        "name": body.name,
                        "version": prompt.version,
                    })),
            );
            (StatusCode::CREATED, Json(serde_json::json!(prompt))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create prompt");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create prompt"}))).into_response()
        }
    }
}

/// `GET /api/v1/prompts` — list all prompt names for tenant.
pub async fn list_names(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    match s.prompt_repo.list_names(auth.0.tenant_id).await {
        Ok(names) => {
            let total = names.len();
            (StatusCode::OK, Json(serde_json::json!({ "prompts": names, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list prompts");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list prompts"}))).into_response()
        }
    }
}

/// `GET /api/v1/prompts/:name/versions` — list all versions of a prompt.
pub async fn list_versions(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match s.prompt_repo.list_versions(auth.0.tenant_id, &name).await {
        Ok(versions) => {
            let total = versions.len();
            (StatusCode::OK, Json(serde_json::json!({ "versions": versions, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list versions");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list versions"}))).into_response()
        }
    }
}

/// `GET /api/v1/prompts/:name/versions/:version` — get a specific version.
pub async fn get_version(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path((name, version)): Path<(String, i32)>,
) -> impl IntoResponse {
    match s.prompt_repo.get_version(auth.0.tenant_id, &name, version).await {
        Ok(Some(p)) => (StatusCode::OK, Json(serde_json::json!(p))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get version");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to get version"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeployRequest {
    #[validate(length(min = 1, max = 64))]
    pub label: String,
    #[validate(range(min = 1))]
    pub version: i32,
}

/// `POST /api/v1/prompts/:name/deploy` — deploy a version to a label.
pub async fn deploy(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(name): Path<String>,
    Json(body): Json<DeployRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    // Confirm the version exists first
    match s.prompt_repo.get_version(auth.0.tenant_id, &name, body.version).await {
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Version not found"}))).into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to verify version");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response();
        }
        _ => {}
    }

    match s
        .prompt_repo
        .deploy(auth.0.tenant_id, &name, &body.label, body.version, Some(auth.0.user_id))
        .await
    {
        Ok(dep) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "prompt")
                    .with_user(auth.0.user_id)
                    .with_resource_id(dep.id.to_string())
                    .with_details(serde_json::json!({
                        "action": "prompt.deploy",
                        "name": name,
                        "label": body.label,
                        "version": body.version,
                    })),
            );
            (StatusCode::OK, Json(serde_json::json!(dep))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to deploy");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to deploy"}))).into_response()
        }
    }
}

/// `GET /api/v1/prompts/:name/deployments` — list all deployments of a prompt.
pub async fn list_deployments(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match s.prompt_repo.list_deployments(auth.0.tenant_id, &name).await {
        Ok(deps) => {
            let total = deps.len();
            (StatusCode::OK, Json(serde_json::json!({ "deployments": deps, "total": total }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list deployments");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list deployments"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ResolveQuery {
    /// Label to resolve (prod/staging/canary). If None → latest version.
    pub label: Option<String>,
    /// Variables for template rendering: `{"name": "Alice"}`
    #[serde(default)]
    pub variables: serde_json::Value,
}

/// `POST /api/v1/prompts/:name/resolve` — resolve name+label into rendered content.
/// This is the **gateway-side** rendering path that clients use when they want
/// the gateway to handle prompt injection. Variables are substituted as `{{var_name}}`.
pub async fn resolve(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(name): Path<String>,
    Json(body): Json<ResolveQuery>,
) -> impl IntoResponse {
    let prompt = match s.prompt_repo.resolve(auth.0.tenant_id, &name, body.label.as_deref()).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Prompt not found"}))).into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to resolve prompt");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response();
        }
    };

    // Render variables: simple `{{var_name}}` substitution
    let rendered = render_template(&prompt.content, &body.variables);

    (StatusCode::OK, Json(serde_json::json!({
        "name": prompt.name,
        "version": prompt.version,
        "content": rendered,
        "model_prefs": prompt.model_prefs,
        "default_model": prompt.default_model,
    }))).into_response()
}

/// Simple `{{var}}` template substitution. Undefined vars are left as-is.
/// Intentionally minimal — if you need loops/conditionals, render client-side.
pub fn render_template(template: &str, vars: &serde_json::Value) -> String {
    let mut out = template.to_string();
    if let Some(obj) = vars.as_object() {
        for (k, v) in obj {
            let placeholder = format!("{{{{{k}}}}}");
            let value_str = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            out = out.replace(&placeholder, &value_str);
        }
    }
    out
}

/// `DELETE /api/v1/prompts/:name/versions/:version` — delete a specific version.
pub async fn delete_version(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path((name, version)): Path<(String, i32)>,
) -> impl IntoResponse {
    match s.prompt_repo.delete_version(auth.0.tenant_id, &name, version).await {
        Ok(_) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "prompt")
                    .with_user(auth.0.user_id)
                    .with_details(serde_json::json!({
                        "action": "prompt.delete_version",
                        "name": name,
                        "version": version,
                    })),
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete version");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete"}))).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_template_basic() {
        let out = render_template("Hello {{name}}, you are {{age}}.", &serde_json::json!({
            "name": "Alice",
            "age": 30,
        }));
        assert_eq!(out, "Hello Alice, you are 30.");
    }

    #[test]
    fn render_template_preserves_undefined() {
        let out = render_template("Hello {{name}}, {{unknown}}.", &serde_json::json!({
            "name": "Bob",
        }));
        assert_eq!(out, "Hello Bob, {{unknown}}.");
    }
}
