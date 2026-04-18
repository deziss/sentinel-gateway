use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Setting {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub key: String,
    pub value: String,
    pub encrypted: bool,
    pub updated_at: DateTime<Utc>,
}
