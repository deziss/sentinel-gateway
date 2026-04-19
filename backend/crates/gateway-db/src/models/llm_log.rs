use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Full LLM request/response log for audit + replay.
/// Request/response bodies are **PII-redacted** before insertion.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LlmLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub virtual_key_id: Option<Uuid>,
    pub backend_id: Option<Uuid>,
    pub model: String,
    pub provider: String,
    pub endpoint_path: String,
    pub request: serde_json::Value,
    pub response: Option<serde_json::Value>,
    pub status_code: i32,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
    pub latency_ms: i64,
    pub cached: bool,
    pub pii_detected: bool,
    pub error: Option<String>,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateLlmLog {
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub virtual_key_id: Option<Uuid>,
    pub backend_id: Option<Uuid>,
    pub model: String,
    pub provider: String,
    pub endpoint_path: String,
    pub request: serde_json::Value,
    pub response: Option<serde_json::Value>,
    pub status_code: i32,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
    pub latency_ms: i64,
    pub cached: bool,
    pub pii_detected: bool,
    pub error: Option<String>,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
}
