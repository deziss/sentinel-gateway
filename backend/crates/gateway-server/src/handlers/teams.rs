use axum::{Json, extract::{State, Path}, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::team::CreateTeam;
use gateway_audit::events::{AuditEvent, EventType};

/// `GET /api/v1/teams`
pub async fn list(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    match s.team_repo.list_by_tenant(auth.0.tenant_id).await {
        Ok(teams) => (StatusCode::OK, Json(serde_json::json!({"teams": teams, "total": teams.len()}))).into_response(),
        Err(e) => {
            tracing::error!("List teams failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list teams"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTeamRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 1, max = 63))]
    pub slug: String,
    pub description: Option<String>,
}

/// `POST /api/v1/teams` *(TenantAdmin+)*
pub async fn create(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateTeamRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let input = CreateTeam {
        tenant_id: auth.0.tenant_id,
        name: body.name.clone(),
        slug: body.slug.clone(),
        description: body.description.clone(),
    };

    match s.team_repo.create(input).await {
        Ok(team) => {
            s.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::TenantCreated, "team")
                    .with_user(auth.0.user_id)
                    .with_resource_id(team.id.to_string())
                    .with_details(serde_json::json!({"name": body.name, "slug": body.slug}))
            );
            (StatusCode::CREATED, Json(serde_json::json!(team))).into_response()
        }
        Err(e) => {
            tracing::error!("Create team failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create team"}))).into_response()
        }
    }
}

/// `GET /api/v1/teams/:id`
pub async fn get(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match s.team_repo.find_by_id(id, auth.0.tenant_id).await {
        Ok(team) => (StatusCode::OK, Json(serde_json::json!(team))).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Team not found"}))).into_response(),
    }
}

/// `DELETE /api/v1/teams/:id`
pub async fn delete(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match s.team_repo.delete(id, auth.0.tenant_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Delete team failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to delete team"}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    #[serde(default = "default_role")]
    #[validate(length(min = 1, max = 20))]
    pub role: String,
}
fn default_role() -> String { "member".to_string() }

/// `POST /api/v1/teams/:id/members`
pub async fn add_member(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> impl IntoResponse {
    if body.role != "owner" && body.role != "admin" && body.role != "member" && body.role != "viewer" {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid role"}))).into_response();
    }
    // Verify team belongs to tenant
    if s.team_repo.find_by_id(id, auth.0.tenant_id).await.is_err() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Team not found"}))).into_response();
    }
    match s.team_repo.add_member(id, body.user_id, &body.role).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "added"}))).into_response(),
        Err(e) => {
            tracing::error!("Add member failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to add member"}))).into_response()
        }
    }
}

/// `DELETE /api/v1/teams/:id/members/:user_id`
pub async fn remove_member(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    if s.team_repo.find_by_id(id, auth.0.tenant_id).await.is_err() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Team not found"}))).into_response();
    }
    match s.team_repo.remove_member(id, user_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Remove member failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to remove member"}))).into_response()
        }
    }
}

/// `GET /api/v1/teams/:id/members`
pub async fn list_members(
    State(s): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if s.team_repo.find_by_id(id, auth.0.tenant_id).await.is_err() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Team not found"}))).into_response();
    }
    match s.team_repo.list_members(id).await {
        Ok(members) => (StatusCode::OK, Json(serde_json::json!({"members": members}))).into_response(),
        Err(e) => {
            tracing::error!("List members failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to list members"}))).into_response()
        }
    }
}
