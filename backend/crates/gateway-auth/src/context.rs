use crate::jwt::Claims;
use crate::roles::Role;
use uuid::Uuid;

/// How the request was authenticated.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Authenticated via JWT Bearer token.
    Jwt { jti: String },
    /// Authenticated via API key.
    ApiKey { key_id: Uuid, scopes: Vec<String> },
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

    /// Check whether this identity has a given scope.
    /// JWT-authenticated users have all scopes. API key users are limited to
    /// the scopes assigned to the key.
    pub fn has_scope(&self, scope: &str) -> bool {
        match &self.method {
            AuthMethod::Jwt { .. } => true,
            AuthMethod::ApiKey { scopes, .. } => scopes.iter().any(|s| s == scope),
        }
    }
}
