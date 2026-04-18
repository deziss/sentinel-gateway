use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "backend_provider_type", rename_all = "snake_case")]
pub enum BackendProviderType {
    OpenAi,
    Anthropic,
    GoogleVertex,
    AwsBedrock,
    Ollama,
    Vllm,
    OpenAiCompatible,
    Qwen,
    Xai,
    Zai,
    // P1 catalog expansion — all OpenAI-compatible
    Mistral,
    Cohere,
    DeepSeek,
    Groq,
    Together,
    Perplexity,
    Fireworks,
    // Generic protocols
    Rest,
    Graphql,
    Grpc,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "health_status", rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Provider data-handling policy — used by tenants to exclude providers that
/// don't meet minimum data-handling requirements. Ordered by strictness:
/// `Strict` > `NoTraining` > `NoRetention` > `Standard`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq, PartialOrd, Ord)]
#[sqlx(type_name = "data_policy", rename_all = "snake_case")]
pub enum DataPolicy {
    /// Provider's default policy (may log for abuse detection).
    Standard,
    /// 0-day retention (OpenAI ZDR, Anthropic zero retention).
    NoRetention,
    /// Contractually won't train on submitted data.
    NoTraining,
    /// Zero logging, zero retention, no training, on-prem only.
    Strict,
}

impl Default for DataPolicy {
    fn default() -> Self {
        Self::Standard
    }
}

impl DataPolicy {
    /// Whether this policy satisfies the required minimum.
    /// Self is "at least as strict as" `required`.
    pub fn satisfies(self, required: DataPolicy) -> bool {
        self >= required
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Backend {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub provider_type: BackendProviderType,
    pub endpoint: String,
    pub encrypted_credentials: Option<String>,
    pub health_status: HealthStatus,
    pub priority: i32,
    pub weight: i32,
    pub timeout_ms: i32,
    pub max_retries: i32,
    pub is_active: bool,
    pub last_health_check: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub data_policy: DataPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackend {
    pub tenant_id: Uuid,
    pub name: String,
    pub provider_type: BackendProviderType,
    pub endpoint: String,
    pub encrypted_credentials: Option<String>,
    pub priority: i32,
    pub weight: i32,
    pub timeout_ms: i32,
    pub max_retries: i32,
}
