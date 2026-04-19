use crate::jwt::Claims;
use crate::roles::Role;
use uuid::Uuid;

/// How the request was authenticated.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Authenticated via JWT Bearer token.
    Jwt { jti: String },
    /// Authenticated via internal API key (sg_*).
    ApiKey { key_id: Uuid, scopes: Vec<String> },
    /// Authenticated via virtual key (vk_*) — maps to a specific backend.
    VirtualKey {
        vkey_id: Uuid,
        backend_id: Uuid,
        team_id: Option<Uuid>,
        allowed_models: Option<Vec<String>>,
        rate_limit_rpm: Option<i32>,
        budget_daily: Option<f64>,
        budget_monthly: Option<f64>,
    },
}

impl AuthMethod {
    /// Return the virtual key ID if the caller authenticated with one.
    pub fn virtual_key_id(&self) -> Option<Uuid> {
        match self {
            AuthMethod::VirtualKey { vkey_id, .. } => Some(*vkey_id),
            _ => None,
        }
    }

    /// Return the API key ID if the caller authenticated with one.
    pub fn api_key_id(&self) -> Option<Uuid> {
        match self {
            AuthMethod::ApiKey { key_id, .. } => Some(*key_id),
            _ => None,
        }
    }
}

/// Unified identity context injected into every authenticated request.
///
/// Both JWT and API key authentication paths produce this same type,
/// so downstream handlers and middleware work uniformly.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub role: Role,
    pub method: AuthMethod,
}

impl AuthContext {
    /// Build from validated JWT claims.
    pub fn from_jwt_claims(claims: &Claims) -> Self {
        Self {
            user_id: claims.user_id(),
            tenant_id: claims.tenant_id(),
            role: claims.role.clone(),
            method: AuthMethod::Jwt {
                jti: claims.jti.clone(),
            },
        }
    }

    /// Build from a resolved API key and its owner's role.
    pub fn from_api_key(key_id: Uuid, user_id: Uuid, tenant_id: Uuid, role: Role, scopes: Vec<String>) -> Self {
        Self {
            user_id,
            tenant_id,
            role,
            method: AuthMethod::ApiKey { key_id, scopes },
        }
    }

    /// Build from a resolved virtual key.
    #[allow(clippy::too_many_arguments)]
    pub fn from_virtual_key(
        vkey_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        backend_id: Uuid,
        team_id: Option<Uuid>,
        allowed_models: Option<Vec<String>>,
        rate_limit_rpm: Option<i32>,
        budget_daily: Option<f64>,
        budget_monthly: Option<f64>,
    ) -> Self {
        Self {
            user_id,
            tenant_id,
            role: Role::User,
            method: AuthMethod::VirtualKey {
                vkey_id,
                backend_id,
                team_id,
                allowed_models,
                rate_limit_rpm,
                budget_daily,
                budget_monthly,
            },
        }
    }

    /// Check whether this identity has a given scope.
    /// JWT-authenticated users have all scopes. API key users are limited to
    /// the scopes assigned to the key.
    pub fn has_scope(&self, scope: &str) -> bool {
        match &self.method {
            AuthMethod::Jwt { .. } => true,
            AuthMethod::ApiKey { scopes, .. } => scopes.iter().any(|s| s == scope),
            AuthMethod::VirtualKey { .. } => true,
        }
    }

    /// Return the backend_id this auth method is bound to (virtual keys only).
    pub fn pinned_backend(&self) -> Option<Uuid> {
        match &self.method {
            AuthMethod::VirtualKey { backend_id, .. } => Some(*backend_id),
            _ => None,
        }
    }

    /// Return the allowed models for this auth method (virtual keys only).
    pub fn allowed_models(&self) -> Option<&[String]> {
        match &self.method {
            AuthMethod::VirtualKey { allowed_models: Some(m), .. } => Some(m.as_slice()),
            _ => None,
        }
    }
}
