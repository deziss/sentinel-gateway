use crate::error::{DbError, DbResult};
use crate::models::backend::{Backend, CreateBackend, HealthStatus};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct BackendRepository {
    pool: DbPool,
}

impl BackendRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateBackend) -> DbResult<Backend> {
        let id = Uuid::new_v4();
        let backend = sqlx::query_as::<_, Backend>(
            r#"
            INSERT INTO backends (id, tenant_id, name, provider_type, endpoint,
                                  encrypted_credentials, priority, weight, timeout_ms, max_retries)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, tenant_id, name, provider_type,
                      endpoint, encrypted_credentials,
                      health_status,
                      priority, weight, timeout_ms, max_retries, is_active,
                      last_health_check, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.name)
        .bind(input.provider_type)
        .bind(input.endpoint)
        .bind(input.encrypted_credentials)
        .bind(input.priority)
        .bind(input.weight)
        .bind(input.timeout_ms)
        .bind(input.max_retries)
        .fetch_one(&self.pool)
        .await?;
        Ok(backend)
    }

    pub async fn find_by_id(&self, id: Uuid, tenant_id: Uuid) -> DbResult<Backend> {
        sqlx::query_as::<_, Backend>(
            r#"
            SELECT id, tenant_id, name, provider_type,
                   endpoint, encrypted_credentials,
                   health_status,
                   priority, weight, timeout_ms, max_retries, is_active,
                   last_health_check, created_at, updated_at
            FROM backends WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Backend {id}")))
    }

    pub async fn list_active_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<Backend>> {
        let backends = sqlx::query_as::<_, Backend>(
            r#"
            SELECT id, tenant_id, name, provider_type,
                   endpoint, encrypted_credentials,
                   health_status,
                   priority, weight, timeout_ms, max_retries, is_active,
                   last_health_check, created_at, updated_at
            FROM backends WHERE tenant_id = $1 AND is_active = true
            ORDER BY priority ASC, weight DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(backends)
    }

    pub async fn update_health(
        &self,
        id: Uuid,
        status: HealthStatus,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE backends
            SET health_status = $1, last_health_check = NOW(), updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_all(&self) -> DbResult<Vec<Backend>> {
        let backends = sqlx::query_as::<_, Backend>(
            r#"
            SELECT id, tenant_id, name, provider_type,
                   endpoint, encrypted_credentials,
                   health_status,
                   priority, weight, timeout_ms, max_retries, is_active,
                   last_health_check, created_at, updated_at
            FROM backends
            ORDER BY tenant_id, priority ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(backends)
    }

    pub async fn count_by_tenant(&self, tenant_id: Uuid) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM backends WHERE tenant_id = $1 AND is_active = true",
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn delete(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query(
            "UPDATE backends SET is_active = false, updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
