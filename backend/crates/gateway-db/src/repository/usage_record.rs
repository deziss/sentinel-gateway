use crate::error::DbResult;
use crate::models::usage_record::{CreateUsageRecord, UsageRecord};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct UsageRecordRepository {
    pool: DbPool,
}

impl UsageRecordRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateUsageRecord) -> DbResult<UsageRecord> {
        let id = Uuid::new_v4();
        let record = sqlx::query_as::<_, UsageRecord>(
            r#"
            INSERT INTO usage_records (id, tenant_id, user_id, api_key_id, backend_id,
                                       model, tokens_input, tokens_output, cost_usd,
                                       latency_ms, status_code, error)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id, tenant_id, user_id, api_key_id, backend_id,
                      model, tokens_input, tokens_output, cost_usd,
                      latency_ms, status_code, error, created_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.user_id)
        .bind(input.api_key_id)
        .bind(input.backend_id)
        .bind(input.model)
        .bind(input.tokens_input)
        .bind(input.tokens_output)
        .bind(input.cost_usd)
        .bind(input.latency_ms)
        .bind(input.status_code)
        .bind(input.error)
        .fetch_one(&self.pool)
        .await?;
        Ok(record)
    }

    pub async fn sum_cost_by_tenant_today(&self, tenant_id: Uuid) -> DbResult<f64> {
        let row: (Option<f64>,) = sqlx::query_as(
            r#"
            SELECT SUM(cost_usd)
            FROM usage_records
            WHERE tenant_id = $1 AND created_at >= NOW() - INTERVAL '1 day'
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.unwrap_or(0.0))
    }

    pub async fn sum_by_tenant_30d(&self, tenant_id: Uuid) -> DbResult<(i64, i64, i64, f64)> {
        // Returns (total_requests, total_tokens_in, total_tokens_out, total_cost)
        let row: (Option<i64>, Option<i64>, Option<i64>, Option<f64>) = sqlx::query_as(
            r#"
            SELECT COUNT(*), SUM(tokens_input), SUM(tokens_output), SUM(cost_usd)
            FROM usage_records
            WHERE tenant_id = $1 AND created_at >= NOW() - INTERVAL '30 days'
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((
            row.0.unwrap_or(0),
            row.1.unwrap_or(0),
            row.2.unwrap_or(0),
            row.3.unwrap_or(0.0),
        ))
    }

    /// Cursor-based pagination. See audit_log.list_by_tenant_cursor for details.
    pub async fn list_by_tenant_cursor(
        &self,
        tenant_id: Uuid,
        cursor_ts: Option<chrono::DateTime<chrono::Utc>>,
        cursor_id: Option<Uuid>,
        limit: i64,
    ) -> DbResult<Vec<UsageRecord>> {
        let records = sqlx::query_as::<_, UsageRecord>(
            r#"
            SELECT id, tenant_id, user_id, api_key_id, backend_id,
                   model, tokens_input, tokens_output, cost_usd,
                   latency_ms, status_code, error, created_at
            FROM usage_records
            WHERE tenant_id = $1
              AND ($2 IS NULL OR created_at < $2
                   OR (created_at = $2 AND ($3 IS NULL OR id < $3)))
            ORDER BY created_at DESC, id DESC
            LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(cursor_ts)
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(records)
    }

    pub async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> DbResult<Vec<UsageRecord>> {
        let records = sqlx::query_as::<_, UsageRecord>(
            r#"
            SELECT id, tenant_id, user_id, api_key_id, backend_id,
                   model, tokens_input, tokens_output, cost_usd,
                   latency_ms, status_code, error, created_at
            FROM usage_records WHERE tenant_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(records)
    }

    pub async fn get_unreported_usage(
        &self,
        tenant_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> DbResult<Vec<crate::models::usage_record::UsageRecordAggregation>> {
        // `SUM(bigint)` returns NUMERIC in Postgres; cast back to BIGINT so sqlx
        // binds cleanly to i64. `provider_type` is the `backend_provider_type` enum —
        // cast to TEXT to deserialize into the String field on UsageRecordAggregation.
        let records = sqlx::query_as::<_, crate::models::usage_record::UsageRecordAggregation>(
            r#"
            SELECT
                u.model                                   AS model,
                b.provider_type::text                     AS provider,
                COUNT(*)::bigint                          AS total_requests,
                COALESCE(SUM(u.tokens_input), 0)::bigint  AS total_tokens_input,
                COALESCE(SUM(u.tokens_output), 0)::bigint AS total_tokens_output,
                COALESCE(SUM(u.cost_usd), 0)::float8      AS total_cost,
                MIN(u.created_at)                         AS start_time,
                MAX(u.created_at)                         AS end_time
            FROM usage_records u
            JOIN backends b ON u.backend_id = b.id
            WHERE u.tenant_id = $1 AND u.created_at > $2
            GROUP BY u.model, b.provider_type
            "#,
        )
        .bind(tenant_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(records)
    }
}
