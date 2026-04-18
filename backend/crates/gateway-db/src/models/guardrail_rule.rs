use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A configured guardrail rule for a tenant.
///
/// The `kind` field determines which built-in or external guardrail implementation
/// to use. The `config` JSONB holds parameters specific to that kind.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GuardrailRule {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub kind: String,
    pub stage: String,
    pub mode: String,
    pub category: String,
    pub config: serde_json::Value,
    pub priority: i32,
    pub is_active: bool,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGuardrailRule {
    pub tenant_id: Uuid,
    pub name: String,
    pub kind: String,
    pub stage: String,
    pub mode: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default = "default_priority")]
    pub priority: i32,
    pub created_by: Option<Uuid>,
}

fn default_category() -> String { "general".to_string() }
fn default_priority() -> i32 { 100 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateGuardrailRule {
    pub kind: Option<String>,
    pub stage: Option<String>,
    pub mode: Option<String>,
    pub category: Option<String>,
    pub config: Option<serde_json::Value>,
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
}
