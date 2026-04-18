use crate::error::DbResult;
use crate::models::audit_log::{AuditLog, CreateAuditLog};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct AuditLogRepository {
    pool: DbPool,
}

impl AuditLogRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateAuditLog) -> DbResult<AuditLog> {
        let id = Uuid::new_v4();
        let log = sqlx::query_as::<_, AuditLog>(
            r#"
            INSERT INTO audit_logs (id, tenant_id, user_id, action, resource_type,
                                   resource_id, details, ip_address, user_agent)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, tenant_id, user_id, action, resource_type,
                      resource_id, details, ip_address, user_agent, created_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.user_id)
        .bind(input.action)
        .bind(input.resource_type)
        .bind(input.resource_id)
        .bind(input.details)
        .bind(input.ip_address)
        .bind(input.user_agent)
        .fetch_one(&self.pool)
        .await?;
        Ok(log)
    }

    pub async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        action: Option<String>,
        resource_type: Option<String>,
        limit: i64,
        offset: i64,
    ) -> DbResult<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, tenant_id, user_id, action, resource_type,
                   resource_id, details, ip_address, user_agent, created_at
            FROM audit_logs 
            WHERE tenant_id = $1
              AND ($2 IS NULL OR user_id = $2)
              AND ($3 IS NULL OR action = $3)
              AND ($4 IS NULL OR resource_type = $4)
            ORDER BY created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(action)
        .bind(resource_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    pub async fn count_by_tenant(
        &self,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        action: Option<String>,
        resource_type: Option<String>,
    ) -> DbResult<i64> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM audit_logs 
            WHERE tenant_id = $1
              AND ($2 IS NULL OR user_id = $2)
              AND ($3 IS NULL OR action = $3)
              AND ($4 IS NULL OR resource_type = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(action)
        .bind(resource_type)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
    }

    /// Cursor-based pagination: fetch rows created before `cursor_ts`.
    /// If both cursor_ts and cursor_id provided, use them as a composite cursor for tie-breaking.
    /// Avoids OFFSET scan. Returns rows in (created_at DESC, id DESC) order.
    pub async fn list_by_tenant_cursor(
        &self,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        action: Option<String>,
        resource_type: Option<String>,
        cursor_ts: Option<chrono::DateTime<chrono::Utc>>,
        cursor_id: Option<Uuid>,
        limit: i64,
    ) -> DbResult<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, tenant_id, user_id, action, resource_type,
                   resource_id, details, ip_address, user_agent, created_at
            FROM audit_logs
            WHERE tenant_id = $1
              AND ($2 IS NULL OR user_id = $2)
              AND ($3 IS NULL OR action = $3)
              AND ($4 IS NULL OR resource_type = $4)
              AND ($5 IS NULL OR created_at < $5
                   OR (created_at = $5 AND ($6 IS NULL OR id < $6)))
            ORDER BY created_at DESC, id DESC
            LIMIT $7
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(action)
        .bind(resource_type)
        .bind(cursor_ts)
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    pub async fn batch_insert(&self, logs: Vec<CreateAuditLog>) -> DbResult<()> {
        let mut tx = self.pool.begin().await?;
        for input in logs {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO audit_logs (id, tenant_id, user_id, action, resource_type,
                                       resource_id, details, ip_address, user_agent)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(id)
            .bind(input.tenant_id)
            .bind(input.user_id)
            .bind(input.action)
            .bind(input.resource_type)
            .bind(input.resource_id)
            .bind(input.details)
            .bind(input.ip_address)
            .bind(input.user_agent)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
