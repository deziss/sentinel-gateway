use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Audit write error: {0}")]
    WriteError(String),

    #[error("Webhook delivery failed: {0}")]
    WebhookFailed(String),

    #[error("Database error: {0}")]
    Database(#[from] gateway_db::DbError),
}
