use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub backend_id: Uuid,
    pub model: Option<String>,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
    pub latency_ms: i64,
    pub status_code: i32,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUsageRecord {
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub backend_id: Uuid,
    pub model: Option<String>,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
    pub latency_ms: i64,
    pub status_code: i32,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageRecordAggregation {
    pub model: Option<String>,
    pub provider: String,
    pub total_requests: i64,
    pub total_tokens_input: i64,
    pub total_tokens_output: i64,
    pub total_cost: f64,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}
