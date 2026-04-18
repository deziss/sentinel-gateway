use crate::error::DbResult;
use crate::models::webhook_failure::{CreateWebhookFailure, WebhookFailure};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct WebhookFailureRepository {
    pool: DbPool,
}

impl WebhookFailureRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Record a failed webhook delivery for later retry.
    pub async fn create(&self, input: CreateWebhookFailure) -> DbResult<WebhookFailure> {
        let id = Uuid::new_v4();
        let failure = sqlx::query_as::<_, WebhookFailure>(
            r#"
            INSERT INTO webhook_failures
                (id, tenant_id, endpoint_id, event_type, payload, signature,
                 attempt_count, last_error, last_attempt_at, next_retry_at, status)
            VALUES ($1, $2, $3, $4, $5, $6, 1, $7, NOW(), NOW() + INTERVAL '1 minute', 'pending')
            RETURNING id, tenant_id, endpoint_id, event_type, payload, signature,
                      attempt_count, last_error, last_attempt_at, next_retry_at, status, created_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.endpoint_id)
        .bind(input.event_type)
        .bind(input.payload)
        .bind(input.signature)
        .bind(input.last_error)
        .fetch_one(&self.pool)
        .await?;
        Ok(failure)
    }

    /// Fetch pending webhook failures ready for retry (up to `limit`).
    /// Uses SKIP LOCKED for safe concurrent worker polling.
    pub async fn claim_retries(&self, limit: i64) -> DbResult<Vec<WebhookFailure>> {
        let failures = sqlx::query_as::<_, WebhookFailure>(
            r#"
            UPDATE webhook_failures
            SET status = 'retrying', last_attempt_at = NOW()
            WHERE id IN (
                SELECT id FROM webhook_failures
                WHERE status IN ('pending', 'retrying')
                  AND next_retry_at <= NOW()
                  AND attempt_count < 10
                ORDER BY next_retry_at
                FOR UPDATE SKIP LOCKED
                LIMIT $1
            )
            RETURNING id, tenant_id, endpoint_id, event_type, payload, signature,
                      attempt_count, last_error, last_attempt_at, next_retry_at, status, created_at
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(failures)
    }

    /// Mark a failure as successfully retried.
    pub async fn mark_delivered(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE webhook_failures SET status = 'delivered' WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Mark a retry as failed and schedule next attempt (exponential backoff).
    pub async fn mark_retry_failed(&self, id: Uuid, error: String) -> DbResult<()> {
        // Exponential backoff: 1min * 2^attempt_count, capped at 24h
        sqlx::query(
            r#"
            UPDATE webhook_failures
            SET attempt_count = attempt_count + 1,
                last_error = $2,
                last_attempt_at = NOW(),
                next_retry_at = NOW() + (LEAST(86400, 60 * POWER(2, attempt_count)) || ' seconds')::INTERVAL,
                status = CASE
                    WHEN attempt_count + 1 >= 10 THEN 'abandoned'
                    ELSE 'pending'
                END
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List recent failures for a tenant (for admin UI).
    pub async fn list_by_tenant(&self, tenant_id: Uuid, limit: i64) -> DbResult<Vec<WebhookFailure>> {
        let failures = sqlx::query_as::<_, WebhookFailure>(
            r#"
            SELECT id, tenant_id, endpoint_id, event_type, payload, signature,
                   attempt_count, last_error, last_attempt_at, next_retry_at, status, created_at
            FROM webhook_failures
            WHERE tenant_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(failures)
    }

    /// Force an immediate retry of a specific failure.
    pub async fn requeue(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE webhook_failures
            SET status = 'pending', next_retry_at = NOW(), attempt_count = 0
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
