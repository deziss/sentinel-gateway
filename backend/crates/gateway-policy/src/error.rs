use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Rate limit exceeded: retry after {retry_after}s")]
    RateLimited {
        retry_after: u64,
        limit: u32,
        remaining: u32,
    },

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("IP address blocked")]
    IpBlocked,

    #[error("Request too large: {size} bytes (max {max})")]
    RequestTooLarge { size: usize, max: usize },

    #[error("GraphQL query depth {depth} exceeds limit {limit}")]
    GraphqlDepthExceeded { depth: u32, limit: u32 },

    #[error("Internal policy error: {0}")]
    Internal(String),
}

impl PolicyError {
    pub fn status_code(&self) -> u16 {
        match self {
            PolicyError::RateLimited { .. } => 429,
            PolicyError::BudgetExceeded(_) => 402,
            PolicyError::IpBlocked => 403,
            PolicyError::RequestTooLarge { .. } => 413,
            PolicyError::GraphqlDepthExceeded { .. } => 400,
            PolicyError::Internal(_) => 500,
        }
    }

    /// Return rate limit headers (X-RateLimit-*) if this is a rate limit error.
    pub fn rate_limit_headers(&self) -> Option<Vec<(String, String)>> {
        match self {
            PolicyError::RateLimited { retry_after, limit, remaining } => {
                Some(vec![
                    ("X-RateLimit-Limit".to_string(), limit.to_string()),
                    ("X-RateLimit-Remaining".to_string(), remaining.to_string()),
                    ("Retry-After".to_string(), retry_after.to_string()),
                ])
            }
            _ => None,
        }
    }
}
