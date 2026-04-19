use crate::error::DbResult;
use crate::models::llm_feedback::{CreateLlmFeedback, LlmFeedback};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct LlmFeedbackRepository {
    pool: DbPool,
}

impl LlmFeedbackRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateLlmFeedback) -> DbResult<LlmFeedback> {
        let id = Uuid::new_v4();
        let fb = sqlx::query_as::<_, LlmFeedback>(
            r#"
            INSERT INTO llm_feedback
                (id, tenant_id, user_id, llm_log_id, request_id, rating, comment, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, COALESCE($8, '{}'::jsonb))
            RETURNING id, tenant_id, user_id, llm_log_id, request_id, rating, comment, metadata, created_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.user_id)
        .bind(input.llm_log_id)
        .bind(&input.request_id)
        .bind(input.rating)
        .bind(&input.comment)
        .bind(&input.metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(fb)
    }

    pub async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        limit: i64,
    ) -> DbResult<Vec<LlmFeedback>> {
        let rows = sqlx::query_as::<_, LlmFeedback>(
            r#"
            SELECT id, tenant_id, user_id, llm_log_id, request_id, rating, comment, metadata, created_at
            FROM llm_feedback WHERE tenant_id = $1
            ORDER BY created_at DESC LIMIT $2
            "#,
        )
        .bind(tenant_id)
        .bind(limit.min(500))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Aggregate stats: (total, positive, negative) in a window.
    pub async fn stats(&self, tenant_id: Uuid, days: i32) -> DbResult<(i64, i64, i64)> {
        let row: (Option<i64>, Option<i64>, Option<i64>) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE rating != 0)             AS total,
                COUNT(*) FILTER (WHERE rating = 1)              AS positive,
                COUNT(*) FILTER (WHERE rating = -1)             AS negative
            FROM llm_feedback
            WHERE tenant_id = $1 AND created_at >= NOW() - ($2 || ' days')::interval
            "#,
        )
        .bind(tenant_id)
        .bind(days.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok((row.0.unwrap_or(0), row.1.unwrap_or(0), row.2.unwrap_or(0)))
    }
}
