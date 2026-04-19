use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// End-user feedback on an LLM response: thumbs up/down + comment.
/// Keyed to either an `llm_log_id` or an externally-provided `request_id`.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LlmFeedback {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub llm_log_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub rating: i16,
    pub comment: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLlmFeedback {
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub llm_log_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub rating: i16,
    pub comment: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
