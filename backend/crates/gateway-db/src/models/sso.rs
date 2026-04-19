use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SsoProvider {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: String,
    pub display_name: String,
    pub slug: String,
    pub client_id: String,
    #[serde(skip_serializing)]
    pub client_secret: String,
    pub issuer_url: Option<String>,
    pub authorize_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
    pub jwks_url: Option<String>,
    pub scopes: String,
    pub default_role: Option<String>,
    pub auto_provision: bool,
    pub is_active: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSsoProvider {
    pub tenant_id: Uuid,
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SsoIdentity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub provider_username: Option<String>,
    pub raw_profile: Option<serde_json::Value>,
    #[serde(skip_serializing)]
    pub access_token_enc: Option<String>,
    #[serde(skip_serializing)]
    pub refresh_token_enc: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertSsoIdentity {
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub provider_username: Option<String>,
    pub raw_profile: Option<serde_json::Value>,
    pub access_token_enc: Option<String>,
    pub refresh_token_enc: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SsoAuthState {
    pub state: String,
    pub provider_id: Uuid,
    pub code_verifier: Option<String>,
    pub nonce: Option<String>,
    pub redirect_after: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
