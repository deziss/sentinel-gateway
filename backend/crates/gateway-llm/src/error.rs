use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("No LLM backend available for model: {0}")]
    NoBackend(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Token limit exceeded")]
    TokenLimitExceeded,

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("Feature not available in current plan: {0}")]
    FeatureNotAvailable(String),

    #[error("Streaming error: {0}")]
    StreamError(String),

    #[error("Internal LLM error: {0}")]
    Internal(String),
}
