use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TenantLicense {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub license_key: String,
    pub license_type: String,
    pub status: String,
    pub plan: String,
    pub entitlements: serde_json::Value,
    pub fingerprint: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_validated_at: Option<DateTime<Utc>>,
    pub last_reported_at: Option<DateTime<Utc>>,
    pub offline_token: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenantLicense {
    pub tenant_id: Uuid,
    pub license_key: String,
    pub license_type: String,
    pub status: String,
    pub plan: String,
    pub entitlements: serde_json::Value,
    pub fingerprint: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub offline_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTenantLicense {
    pub status: Option<String>,
    pub plan: Option<String>,
    pub entitlements: Option<serde_json::Value>,
    pub fingerprint: Option<String>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub last_validated_at: Option<DateTime<Utc>>,
    pub last_reported_at: Option<DateTime<Utc>>,
    pub offline_token: Option<String>,
}
