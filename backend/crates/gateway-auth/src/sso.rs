//! Provider-agnostic SSO/OIDC integration interface.
//!
//! This module defines the `SsoProvider` trait that concrete SSO providers
//! (Okta, Auth0, Azure AD, Google, etc.) will implement. A `NoopSsoProvider`
//! is included as the default when SSO is not configured.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AuthError;

/// Information returned by an SSO provider after successful authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoUserInfo {
    /// Provider identifier (e.g., "okta", "auth0", "azure_ad").
    pub provider: String,
    /// User ID at the provider.
    pub provider_user_id: String,
    /// User's email address.
    pub email: String,
    /// Display name.
    pub name: Option<String>,
    /// Avatar / profile picture URL.
    pub avatar_url: Option<String>,
    /// Groups or roles from the provider (for mapping to gateway roles).
    pub groups: Vec<String>,
}

/// Configuration for an SSO/OIDC provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoProviderConfig {
    pub provider_name: String,
    pub client_id: String,
    pub client_secret: String,
    /// OIDC discovery URL (e.g., `https://accounts.google.com`).
    pub issuer_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

/// Provider-agnostic trait for SSO integration.
///
/// Implementors handle the OIDC/OAuth2 flow for a specific identity provider.
#[async_trait]
pub trait SsoProvider: Send + Sync {
    /// Return the authorization URL to redirect the user to.
    fn authorization_url(&self, state: &str, nonce: &str) -> Result<String, AuthError>;

    /// Exchange an authorization code for user info (authorization code flow).
    async fn exchange_code(&self, code: &str) -> Result<SsoUserInfo, AuthError>;

    /// Validate an ID token and extract user info (implicit flow).
    async fn validate_id_token(&self, id_token: &str) -> Result<SsoUserInfo, AuthError>;

    /// Provider name for logging and configuration.
    fn provider_name(&self) -> &str;
}

/// Placeholder implementation that always returns an error.
/// Used when SSO is not configured.
pub struct NoopSsoProvider;

#[async_trait]
impl SsoProvider for NoopSsoProvider {
    fn authorization_url(&self, _state: &str, _nonce: &str) -> Result<String, AuthError> {
        Err(AuthError::Internal("SSO not configured".into()))
    }

    async fn exchange_code(&self, _code: &str) -> Result<SsoUserInfo, AuthError> {
        Err(AuthError::Internal("SSO not configured".into()))
    }

    async fn validate_id_token(&self, _id_token: &str) -> Result<SsoUserInfo, AuthError> {
        Err(AuthError::Internal("SSO not configured".into()))
    }

    fn provider_name(&self) -> &str {
        "noop"
    }
}
