use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Virtual Key — consumer-facing key that maps to a backend + per-key policies.
///
/// Portkey-pattern: callers get the virtual key (can be rotated/revoked),
/// never see the real provider credentials. Each virtual key can have its own
/// rate limits, budgets, and model allow-list.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VirtualKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub team_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub name: String,
    #[serde(skip_serializing)]
    pub key_hash: String,
    pub key_prefix: String,
    pub backend_id: Uuid,
    pub allowed_models: Option<Vec<String>>,
    pub rate_limit_rpm: Option<i32>,
    pub token_limit_tpm: Option<i32>,
    pub budget_daily: Option<f64>,
    pub budget_monthly: Option<f64>,
    pub metadata: serde_json::Value,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVirtualKey {
    pub tenant_id: Uuid,
    pub team_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub backend_id: Uuid,
    pub allowed_models: Option<Vec<String>>,
    pub rate_limit_rpm: Option<i32>,
    pub token_limit_tpm: Option<i32>,
    pub budget_daily: Option<f64>,
    pub budget_monthly: Option<f64>,
    pub metadata: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
}
