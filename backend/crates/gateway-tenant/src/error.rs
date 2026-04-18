use thiserror::Error;

#[derive(Debug, Error)]
pub enum TenantError {
    #[error("Tenant not found")]
    NotFound,

    #[error("Tenant resolution failed: {0}")]
    ResolutionFailed(String),

    #[error("Tenant inactive")]
    Inactive,

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
