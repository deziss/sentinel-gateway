//! Organizations — parent grouping for multiple tenants (Portkey-style).
//! SuperAdmin only; tenants may be re-parented under an org for billing & views.
//!
//!   GET    /api/v1/organizations
//!   POST   /api/v1/organizations
//!   GET    /api/v1/organizations/:id
//!   DELETE /api/v1/organizations/:id
//!   POST   /api/v1/organizations/:id/tenants/:tenant_id  (assign)
//!   GET    /api/v1/organizations/:id/tenants             (list tenants)

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::handlers::feature_gate::{require_feature, require_super_admin};
use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_db::models::organization::CreateOrganization;
use gateway_license::Feature;

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub slug: String,
    pub name: String,
    pub plan: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    if let Err(resp) = require_feature(&state, &auth.0, Feature::OrgManagement).await { return resp; }
    match state.organization_repo.list().await {
        Ok(orgs) => (StatusCode::OK, Json(orgs)).into_response(),
        Err(e) => err_500(e),
    }
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    if let Err(resp) = require_feature(&state, &auth.0, Feature::OrgManagement).await { return resp; }
    let input = CreateOrganization {
        slug: body.slug,
        name: body.name,
        plan: body.plan,
        metadata: body.metadata,
    };
    match state.organization_repo.create(input).await {
        Ok(org) => (StatusCode::CREATED, Json(org)).into_response(),
        Err(e) => err_500(e),
    }
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    match state.organization_repo.find_by_id(id).await {
        Ok(org) => (StatusCode::OK, Json(org)).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    if let Err(resp) = require_feature(&state, &auth.0, Feature::OrgManagement).await { return resp; }
    match state.organization_repo.delete(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_500(e),
    }
}

pub async fn assign_tenant(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path((id, tenant_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    match state.organization_repo.assign_tenant(tenant_id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_500(e),
    }
}

pub async fn list_tenants(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&auth) { return resp; }
    match state.organization_repo.list_tenants(id).await {
        Ok(ids) => (StatusCode::OK, Json(ids)).into_response(),
        Err(e) => err_500(e),
    }
}

fn err_500(e: gateway_db::error::DbError) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": e.to_string()})),
    )
        .into_response()
}
