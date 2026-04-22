use thiserror::Error;

#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("License key invalid")]
    Invalid,

    #[error("License expired")]
    Expired,

    #[error("License not activated")]
    NotActivated,

    #[error("Feature not available in current plan: {0}")]
    FeatureNotAvailable(String),

    #[error("Hardware fingerprint mismatch")]
    FingerprintMismatch,

    #[error("Internal license error: {0}")]
    Internal(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("License not found: {0}")]
    NotFound(String),
}
