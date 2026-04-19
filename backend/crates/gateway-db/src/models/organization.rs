use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Organization — parent grouping for multiple tenants.
/// Enables Portkey-style "Organization" accounts where one customer can
/// operate several isolated tenants (environments, teams, customers).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub metadata: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrganization {
    pub slug: String,
    pub name: String,
    pub plan: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
