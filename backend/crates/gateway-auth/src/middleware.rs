use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

use crate::{
    api_key::ApiKeyService,
    api_key_cache::ApiKeyCache,
    context::AuthContext,
    error::AuthError,
    jwt::JwtService,
    roles::Role,
    token_blacklist::TokenBlacklist,
};
use gateway_db::repository::{ApiKeyRepository, UserRepository};
use gateway_db::models::user::UserRole;

/// Auth state shared via Axum layer state.
#[derive(Clone)]
pub struct AuthState {
    pub jwt: Arc<JwtService>,
    pub token_blacklist: Arc<TokenBlacklist>,
    pub api_key_cache: Arc<ApiKeyCache>,
    pub api_key_repo: Arc<ApiKeyRepository>,
    pub user_repo: Arc<UserRepository>,
}

/// Axum extension carrying the authenticated identity context.
#[derive(Debug, Clone)]
pub struct RequireAuth(pub AuthContext);

/// Axum extension carrying the required role already checked.
#[derive(Debug, Clone)]
pub struct RequireRole(pub AuthContext);

/// Middleware: validates JWT or API key and injects `RequireAuth` extension.
/// Returns 401 if no valid credentials are provided.
pub async fn auth_middleware(
    State(state): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> Response {
    match authenticate(&state, req.headers()).await {
        Ok(ctx) => {
            req.extensions_mut().insert(RequireAuth(ctx));
            next.run(req).await
        }
        Err(e) => auth_error_response(e),
    }
}

/// Optional auth middleware: same logic, but proceeds without `RequireAuth`
/// if no credentials are provided. Used for the proxy fallback route.
pub async fn optional_auth_middleware(
    State(state): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> Response {
    match authenticate(&state, req.headers()).await {
        Ok(ctx) => {
            req.extensions_mut().insert(RequireAuth(ctx));
        }
        Err(AuthError::Unauthorized(_)) => {
            // No credentials — proceed without auth context
        }
        Err(e) => {
            // Credentials were provided but invalid — reject
            return auth_error_response(e);
        }
    }
    next.run(req).await
}

/// Core authentication logic shared by both middleware variants.
async fn authenticate(state: &AuthState, headers: &HeaderMap) -> Result<AuthContext, AuthError> {
    // Strategy 1: X-API-Key header (explicit API key)
    if let Some(key_str) = headers.get("X-API-Key").and_then(|v| v.to_str().ok()) {
        return authenticate_api_key(state, key_str).await;
    }

    // Strategy 2: Authorization header
    if let Some(auth_header) = headers.get("Authorization").and_then(|v| v.to_str().ok()) {
        if let Some(bearer_value) = auth_header.strip_prefix("Bearer ") {
            // Distinguish API key (sg_ prefix) from JWT
            if bearer_value.starts_with("sg_") {
                return authenticate_api_key(state, bearer_value).await;
            } else {
                return authenticate_jwt(state, bearer_value);
            }
        }
    }

    Err(AuthError::Unauthorized("Missing authentication".into()))
}

/// Validate a JWT Bearer token.
fn authenticate_jwt(state: &AuthState, token: &str) -> Result<AuthContext, AuthError> {
    let claims = state.jwt.validate_with_blacklist(token, &state.token_blacklist)?;

    // Reject refresh tokens used as access tokens
    if claims.typ != "access" {
        return Err(AuthError::TokenInvalid("Expected access token".into()));
    }

    Ok(AuthContext::from_jwt_claims(&claims))
}

/// Validate an API key: cache lookup -> DB fallback -> cache insert.
async fn authenticate_api_key(state: &AuthState, key: &str) -> Result<AuthContext, AuthError> {
    let key_hash = ApiKeyService::hash(key);

    // 1. Try cache
    if let Some(cached) = state.api_key_cache.get(&key_hash) {
        if !cached.is_active {
            return Err(AuthError::ApiKeyInvalid);
        }
        return Ok(AuthContext::from_api_key(
            cached.key_id,
            cached.user_id,
            cached.tenant_id,
            cached.role,
            cached.scopes,
        ));
    }

    // 2. DB lookup
    let api_key = state.api_key_repo
        .find_by_hash(&key_hash)
        .await
        .map_err(|_| AuthError::ApiKeyInvalid)?;

    if !api_key.is_active {
        return Err(AuthError::ApiKeyInvalid);
    }

    // Check expiration
    if let Some(expires_at) = api_key.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(AuthError::ApiKeyInvalid);
        }
    }

    // Resolve user's role
    let user = state.user_repo
        .find_by_id(api_key.user_id, api_key.tenant_id)
        .await
        .map_err(|_| AuthError::ApiKeyInvalid)?;

    let role = db_role_to_auth_role(&user.role);

    // 3. Cache the result
    state.api_key_cache.insert(
        key_hash,
        api_key.id,
        api_key.tenant_id,
        api_key.user_id,
        role.clone(),
        api_key.scopes.clone(),
        api_key.is_active,
        api_key.rate_limit_rpm,
        api_key.budget_daily,
        api_key.budget_monthly,
    );

    // 4. Fire-and-forget: update last_used_at
    let repo = state.api_key_repo.clone();
    let key_id = api_key.id;
    tokio::spawn(async move {
        if let Err(e) = repo.touch_used(key_id).await {
            warn!("Failed to update API key last_used_at: {e}");
        }
    });

    Ok(AuthContext::from_api_key(
        api_key.id,
        api_key.user_id,
        api_key.tenant_id,
        role,
        api_key.scopes,
    ))
}

fn db_role_to_auth_role(role: &UserRole) -> Role {
    match role {
        UserRole::SuperAdmin => Role::SuperAdmin,
        UserRole::TenantAdmin => Role::TenantAdmin,
        UserRole::User => Role::User,
        UserRole::ReadOnly => Role::ReadOnly,
    }
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

fn auth_error_response(err: AuthError) -> Response {
    let status = StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::UNAUTHORIZED);
    (status, Json(json!({ "error": err.to_string() }))).into_response()
}

/// Extractor that returns 403 if the user doesn't have the required role.
pub fn require_role(ctx: &AuthContext, required: &Role) -> Result<(), AuthError> {
    if ctx.role.can(required) {
        Ok(())
    } else {
        Err(AuthError::Forbidden)
    }
}

/// Middleware factory for role-based access control.
pub async fn role_gate(
    req: Request,
    next: Next,
    required_role: Role,
) -> Response {
    let ctx = req.extensions().get::<RequireAuth>().map(|ra| &ra.0);

    match ctx {
        Some(ctx) => {
            if ctx.role.can(&required_role) {
                next.run(req).await
            } else {
                auth_error_response(AuthError::Forbidden)
            }
        }
        None => auth_error_response(AuthError::Unauthorized("Auth context missing".into())),
    }
}
