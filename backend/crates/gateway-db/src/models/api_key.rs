use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub key_hash: String,
    pub name: String,
    pub scopes: Vec<String>,
    pub rate_limit_rpm: Option<i32>,
    pub budget_daily: Option<f64>,
    pub budget_monthly: Option<f64>,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKey {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub key_hash: String,
    pub name: String,
    pub scopes: Vec<String>,
    pub rate_limit_rpm: Option<i32>,
    pub budget_daily: Option<f64>,
    pub budget_monthly: Option<f64>,
    pub expires_at: Option<DateTime<Utc>>,
}
