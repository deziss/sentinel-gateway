//! SSO authentication HTTP handlers (OAuth2 / OIDC).
//!
//! Public endpoints:
//!   - GET  /auth/sso/:slug/authorize  → redirect user to provider
//!   - GET  /auth/sso/:slug/callback   → handle provider redirect, issue JWT
//!
//! Admin endpoints (TenantAdmin+):
//!   - GET    /sso/providers            → list tenant providers
//!   - POST   /sso/providers            → create provider
//!   - DELETE /sso/providers/:id        → soft-delete provider

use axum::{
    Json, Extension,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use gateway_auth::middleware::RequireAuth;
use gateway_auth::roles::Role;
use gateway_auth::sso::{
    apply_provider_defaults, build_provider, generate_pkce, generate_state, OAuth2Config,
    ProviderKind,
};
use gateway_audit::events::{AuditEvent, EventType};
use gateway_db::models::sso::{CreateSsoProvider, UpsertSsoIdentity};
use gateway_db::models::user::{CreateUser, UserRole};
use gateway_license::Feature;

use crate::handlers::feature_gate::require_feature;
use crate::state::AppState;

// ── Query params ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub tenant: String,
    pub redirect_after: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn build_oauth_config(
    provider: &gateway_db::models::SsoProvider,
    redirect_uri: &str,
) -> Result<OAuth2Config, (StatusCode, Json<serde_json::Value>)> {
    let kind = ProviderKind::from_str(&provider.kind)
        .ok_or((StatusCode::BAD_REQUEST, Json(json!({"error": format!("unknown provider kind: {}", provider.kind)}))))?;

    let scopes: Vec<String> = provider
        .scopes
        .split_whitespace()
        .map(String::from)
        .collect();

    Ok(apply_provider_defaults(OAuth2Config {
        kind,
        client_id: provider.client_id.clone(),
        client_secret: provider.client_secret.clone(),
        redirect_uri: redirect_uri.to_string(),
        scopes,
        issuer_url: provider.issuer_url.clone(),
        authorize_url: provider.authorize_url.clone(),
        token_url: provider.token_url.clone(),
        userinfo_url: provider.userinfo_url.clone(),
    }))
}

fn redirect_uri_for(state: &AppState, slug: &str) -> String {
    // Prefer explicit SSO_PUBLIC_BASE_URL env var; fall back to host:port from server config.
    let base = std::env::var("SSO_PUBLIC_BASE_URL").ok().unwrap_or_else(|| {
        let scheme = if state.server_config.require_tls { "https" } else { "http" };
        format!("{scheme}://{}:{}", state.server_config.host, state.server_config.port)
    });
    format!("{}/api/v1/auth/sso/{slug}/callback", base.trim_end_matches('/'))
}

// ── Public: GET /auth/sso/:slug/authorize ───────────────────────────────────

pub async fn authorize(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(q): Query<AuthorizeQuery>,
) -> impl IntoResponse {
    let tenant = match state.tenant_repo.find_by_slug(&q.tenant).await {
        Ok(t) => t,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({"error": "tenant not found"}))).into_response(),
    };

    if let Err(resp) = crate::handlers::feature_gate::require_feature_for_tenant(&state, Some(tenant.id), Feature::Sso).await { return resp; }

    let provider = match state.sso_provider_repo.find_by_slug(tenant.id, &slug).await {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({"error": "SSO provider not found"}))).into_response(),
    };

    let redirect_uri = redirect_uri_for(&state, &slug);

    let cfg = match build_oauth_config(&provider, &redirect_uri) {
        Ok(c) => c,
        Err(resp) => return resp.into_response(),
    };

    let oauth = match build_provider(cfg) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    let state_token = generate_state();
    let (verifier, challenge) = generate_pkce();
    let use_pkce = provider.kind != "github";

    if let Err(e) = state
        .sso_auth_state_repo
        .create(
            &state_token,
            provider.id,
            if use_pkce { Some(verifier.as_str()) } else { None },
            None,
            q.redirect_after.as_deref(),
        )
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response();
    }

    let url = match oauth.build_authorize_url(
        &state_token,
        if use_pkce { Some(&challenge) } else { None },
        None,
    ) {
        Ok(u) => u,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    Redirect::to(&url).into_response()
}

// ── Public: GET /auth/sso/:slug/callback ────────────────────────────────────

pub async fn callback(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(q): Query<CallbackQuery>,
) -> impl IntoResponse {
    if let Some(err) = q.error {
        let desc = q.error_description.unwrap_or_default();
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": err, "description": desc}))).into_response();
    }

    let code = match q.code {
        Some(c) => c,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "missing code"}))).into_response(),
    };
    let state_token = match q.state {
        Some(s) => s,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "missing state"}))).into_response(),
    };

    // 1. Consume state (single-use, atomic)
    let auth_state = match state.sso_auth_state_repo.consume(&state_token).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid or expired state"}))).into_response(),
    };

    // 2. Load provider
    let provider = match state.sso_provider_repo.find_by_id(auth_state.provider_id).await {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({"error": "provider gone"}))).into_response(),
    };

    if let Err(resp) = crate::handlers::feature_gate::require_feature_for_tenant(&state, Some(provider.tenant_id), Feature::Sso).await { return resp; }

    let redirect_uri = redirect_uri_for(&state, &slug);

    let cfg = match build_oauth_config(&provider, &redirect_uri) {
        Ok(c) => c,
        Err(resp) => return resp.into_response(),
    };

    let oauth = match build_provider(cfg) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    // 3. Exchange code
    let result = match oauth
        .exchange_code(&code, auth_state.code_verifier.as_deref())
        .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": e.to_string()}))).into_response(),
    };

    if result.user_info.email.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "provider did not return an email"}))).into_response();
    }

    // 4. Find-or-provision user in this tenant by email
    let user = match state
        .user_repo
        .find_by_email(&result.user_info.email, provider.tenant_id)
        .await
    {
        Ok(u) => u,
        Err(_) if provider.auto_provision => {
            let role = match provider.default_role.as_deref() {
                Some("tenant_admin") => UserRole::TenantAdmin,
                Some("read_only") => UserRole::ReadOnly,
                _ => UserRole::User,
            };
            match state
                .user_repo
                .create(CreateUser {
                    tenant_id: provider.tenant_id,
                    email: result.user_info.email.clone(),
                    // SSO users have no local password — random hash that can never match
                    password_hash: format!("!sso:{}", Uuid::new_v4()),
                    role,
                })
                .await
            {
                Ok(u) => u,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("could not provision user: {e}")})),
                    )
                        .into_response();
                }
            }
        }
        Err(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "user not registered and auto-provisioning is disabled"})),
            )
                .into_response();
        }
    };

    // 5. Upsert SSO identity link
    let _ = state
        .sso_identity_repo
        .upsert(UpsertSsoIdentity {
            user_id: user.id,
            provider_id: provider.id,
            provider_user_id: result.user_info.provider_user_id.clone(),
            provider_email: Some(result.user_info.email.clone()),
            provider_username: result.user_info.username.clone(),
            raw_profile: Some(result.user_info.raw_profile.clone()),
            access_token_enc: None,
            refresh_token_enc: None,
            token_expires_at: None,
        })
        .await;

    // 6. Map DB role → auth Role
    let auth_role = match user.role {
        UserRole::SuperAdmin => Role::SuperAdmin,
        UserRole::TenantAdmin => Role::TenantAdmin,
        UserRole::User => Role::User,
        UserRole::ReadOnly => Role::ReadOnly,
    };

    // 7. Issue JWTs
    let access_token = match state.jwt.issue_access_token(user.id, user.tenant_id, auth_role.clone()) {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "token generation failed"})),
            )
                .into_response();
        }
    };
    let refresh_token = match state.jwt.issue_refresh_token(user.id, user.tenant_id, auth_role) {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "token generation failed"})),
            )
                .into_response();
        }
    };

    // 8. Audit
    state.audit_service.log(
        AuditEvent::new(provider.tenant_id, EventType::UserLogin, "sso")
            .with_user(user.id)
            .with_resource_id(provider.id.to_string())
            .with_details(json!({"method": "sso", "provider": provider.kind, "slug": provider.slug})),
    );

    let body = json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "token_type": "Bearer",
        "expires_in": state.auth_config.access_token_ttl_minutes * 60,
        "user": {
            "id": user.id,
            "email": user.email,
            "tenant_id": user.tenant_id,
            "role": user.role,
        },
        "redirect_after": auth_state.redirect_after,
    });

    (StatusCode::OK, Json(body)).into_response()
}

// ── Admin: SSO provider CRUD (TenantAdmin+) ─────────────────────────────────

pub async fn list_providers(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    if let Err(resp) = require_feature(&state, &auth.0, Feature::Sso).await { return resp; }
    match state.sso_provider_repo.list_by_tenant(auth.0.tenant_id).await {
        Ok(ps) => (StatusCode::OK, Json(ps)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub kind: String,
    pub display_name: String,
    pub slug: String,
    pub client_id: String,
    pub client_secret: String,
    pub issuer_url: Option<String>,
    pub authorize_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
    pub jwks_url: Option<String>,
    pub scopes: Option<String>,
    pub default_role: Option<String>,
    pub auto_provision: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CreateProviderResponse {
    pub id: Uuid,
    pub slug: String,
    pub kind: String,
    pub display_name: String,
}

pub async fn create_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_feature(&state, &auth.0, Feature::Sso).await { return resp; }
    if ProviderKind::from_str(&body.kind).is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("unknown provider kind: {}", body.kind)})),
        )
            .into_response();
    }

    let input = CreateSsoProvider {
        tenant_id: auth.0.tenant_id,
        kind: body.kind.clone(),
        display_name: body.display_name.clone(),
        slug: body.slug.clone(),
        client_id: body.client_id,
        client_secret: body.client_secret,
        issuer_url: body.issuer_url,
        authorize_url: body.authorize_url,
        token_url: body.token_url,
        userinfo_url: body.userinfo_url,
        jwks_url: body.jwks_url,
        scopes: body.scopes,
        default_role: body.default_role,
        auto_provision: body.auto_provision,
        metadata: body.metadata,
    };

    match state.sso_provider_repo.create(input).await {
        Ok(p) => {
            state.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "sso_provider")
                    .with_user(auth.0.user_id)
                    .with_resource_id(p.id.to_string())
                    .with_details(json!({"action": "create", "kind": p.kind, "slug": p.slug})),
            );
            (
                StatusCode::CREATED,
                Json(CreateProviderResponse {
                    id: p.id,
                    slug: p.slug,
                    kind: p.kind,
                    display_name: p.display_name,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(resp) = require_feature(&state, &auth.0, Feature::Sso).await { return resp; }
    match state.sso_provider_repo.delete(id, auth.0.tenant_id).await {
        Ok(()) => {
            state.audit_service.log(
                AuditEvent::new(auth.0.tenant_id, EventType::SettingsChanged, "sso_provider")
                    .with_user(auth.0.user_id)
                    .with_resource_id(id.to_string())
                    .with_details(json!({"action": "delete"})),
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}
