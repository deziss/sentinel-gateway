use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A versioned prompt template.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Prompt {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub version: i32,
    pub content: String,
    /// JSONB: `{ "var_name": { "type": "string", "required": true } }`
    pub variables: serde_json::Value,
    /// JSONB: `{ "temperature": 0.7, "max_tokens": 2048 }`
    pub model_prefs: serde_json::Value,
    pub default_model: Option<String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePrompt {
    pub tenant_id: Uuid,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub variables: serde_json::Value,
    #[serde(default)]
    pub model_prefs: serde_json::Value,
    pub default_model: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
}

/// An active deployment — maps `label` (prod/staging/canary/...) to a specific version.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PromptDeployment {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub prompt_name: String,
    pub label: String,
    pub version: i32,
    pub deployed_by: Option<Uuid>,
    pub deployed_at: DateTime<Utc>,
}
