use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: String,
    pub settings: serde_json::Value,
    pub license_key: Option<String>,
    pub is_active: bool,
    pub max_users: i32,
    pub max_api_keys: i32,
    pub max_backends: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenant {
    pub name: String,
    pub slug: String,
    pub plan: String,
    pub max_users: i32,
    pub max_api_keys: i32,
    pub max_backends: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTenant {
    pub name: Option<String>,
    pub plan: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub max_users: Option<i32>,
    pub max_api_keys: Option<i32>,
    pub max_backends: Option<i32>,
    pub is_active: Option<bool>,
    pub license_key: Option<String>,
}
