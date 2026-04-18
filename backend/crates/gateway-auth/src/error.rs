use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Token expired")]
    TokenExpired,

    #[error("Token invalid: {0}")]
    TokenInvalid(String),

    #[error("Token has been revoked")]
    TokenRevoked,

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Account locked until {0}")]
    AccountLocked(String),

    #[error("API key invalid or revoked")]
    ApiKeyInvalid,

    #[error("Insufficient permissions")]
    Forbidden,

    #[error("Internal auth error: {0}")]
    Internal(String),
}

impl AuthError {
    pub fn status_code(&self) -> u16 {
        match self {
            AuthError::InvalidCredentials | AuthError::ApiKeyInvalid => 401,
            AuthError::TokenExpired | AuthError::TokenInvalid(_) | AuthError::TokenRevoked => 401,
            AuthError::Unauthorized(_) => 401,
            AuthError::Forbidden => 403,
            AuthError::AccountLocked(_) => 423,
            AuthError::Internal(_) => 500,
        }
    }
}
