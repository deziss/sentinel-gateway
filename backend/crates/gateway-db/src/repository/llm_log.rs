use crate::error::DbResult;
use crate::models::llm_log::{CreateLlmLog, LlmLog};
use crate::pool::DbPool;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct LlmLogRepository {
    pool: DbPool,
}

impl LlmLogRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateLlmLog) -> DbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO llm_logs
                (id, tenant_id, user_id, api_key_id, virtual_key_id, backend_id,
                 model, provider, endpoint_path, request, response, status_code,
                 tokens_input, tokens_output, cost_usd, latency_ms, cached,
                 pii_detected, error, trace_id, request_id)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            "#,
        )
        .bind(input.tenant_id)
        .bind(input.user_id)
        .bind(input.api_key_id)
        .bind(input.virtual_key_id)
        .bind(input.backend_id)
        .bind(&input.model)
        .bind(&input.provider)
        .bind(&input.endpoint_path)
        .bind(&input.request)
        .bind(&input.response)
        .bind(input.status_code)
        .bind(input.tokens_input)
        .bind(input.tokens_output)
        .bind(input.cost_usd)
        .bind(input.latency_ms)
        .bind(input.cached)
        .bind(input.pii_detected)
        .bind(&input.error)
        .bind(&input.trace_id)
        .bind(&input.request_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Batch insert for write-behind buffering.
    pub async fn batch_insert(&self, logs: Vec<CreateLlmLog>) -> DbResult<()> {
        if logs.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for log in logs {
            sqlx::query(
                r#"
                INSERT INTO llm_logs
                    (id, tenant_id, user_id, api_key_id, virtual_key_id, backend_id,
                     model, provider, endpoint_path, request, response, status_code,
                     tokens_input, tokens_output, cost_usd, latency_ms, cached,
                     pii_detected, error, trace_id, request_id)
                VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                        $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
                "#,
            )
            .bind(log.tenant_id)
            .bind(log.user_id)
            .bind(log.api_key_id)
            .bind(log.virtual_key_id)
            .bind(log.backend_id)
            .bind(&log.model)
            .bind(&log.provider)
            .bind(&log.endpoint_path)
            .bind(&log.request)
            .bind(&log.response)
            .bind(log.status_code)
            .bind(log.tokens_input)
            .bind(log.tokens_output)
            .bind(log.cost_usd)
            .bind(log.latency_ms)
            .bind(log.cached)
            .bind(log.pii_detected)
            .bind(&log.error)
            .bind(&log.trace_id)
            .bind(&log.request_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Cursor-paginated search with optional filters.
    #[allow(clippy::too_many_arguments)]
    pub async fn search(
        &self,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        model: Option<String>,
        status_min: Option<i32>,
        status_max: Option<i32>,
        cursor_ts: Option<DateTime<Utc>>,
        cursor_id: Option<Uuid>,
        limit: i64,
    ) -> DbResult<Vec<LlmLog>> {
        let logs = sqlx::query_as::<_, LlmLog>(
            r#"
            SELECT id, tenant_id, user_id, api_key_id, virtual_key_id, backend_id,
                   model, provider, endpoint_path, request, response, status_code,
                   tokens_input, tokens_output, cost_usd, latency_ms, cached,
                   pii_detected, error, trace_id, request_id, created_at
            FROM llm_logs
            WHERE tenant_id = $1
              AND ($2 IS NULL OR user_id = $2)
              AND ($3 IS NULL OR model = $3)
              AND ($4 IS NULL OR status_code >= $4)
              AND ($5 IS NULL OR status_code <= $5)
              AND ($6 IS NULL OR created_at < $6
                   OR (created_at = $6 AND ($7 IS NULL OR id < $7)))
            ORDER BY created_at DESC, id DESC
            LIMIT $8
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(model)
        .bind(status_min)
        .bind(status_max)
        .bind(cursor_ts)
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    /// Enforce per-tenant retention: delete logs older than `days`.
    /// Returns the row count deleted.
    pub async fn delete_older_than(&self, tenant_id: Uuid, days: i32) -> DbResult<u64> {
        let result = sqlx::query(
            "DELETE FROM llm_logs WHERE tenant_id = $1 AND created_at < NOW() - ($2 || ' days')::interval"
        )
        .bind(tenant_id)
        .bind(days.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
