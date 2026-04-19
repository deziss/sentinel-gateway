use crate::error::{DbError, DbResult};
use crate::models::virtual_key::{CreateVirtualKey, VirtualKey};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct VirtualKeyRepository {
    pool: DbPool,
}

impl VirtualKeyRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateVirtualKey) -> DbResult<VirtualKey> {
        let id = Uuid::new_v4();
        let vk = sqlx::query_as::<_, VirtualKey>(
            r#"
            INSERT INTO virtual_keys
                (id, tenant_id, team_id, user_id, name, key_hash, key_prefix,
                 backend_id, allowed_models, rate_limit_rpm, token_limit_tpm,
                 budget_daily, budget_monthly, metadata, expires_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
            RETURNING id, tenant_id, team_id, user_id, name, key_hash, key_prefix,
                      backend_id, allowed_models, rate_limit_rpm, token_limit_tpm,
                      budget_daily, budget_monthly, metadata, is_active, expires_at,
                      last_used_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.team_id)
        .bind(input.user_id)
        .bind(input.name)
        .bind(input.key_hash)
        .bind(input.key_prefix)
        .bind(input.backend_id)
        .bind(input.allowed_models.as_deref())
        .bind(input.rate_limit_rpm)
        .bind(input.token_limit_tpm)
        .bind(input.budget_daily)
        .bind(input.budget_monthly)
        .bind(input.metadata)
        .bind(input.expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(vk)
    }

    /// Hot-path lookup by hash (partial index `WHERE is_active=true`).
    pub async fn find_by_hash(&self, key_hash: &str) -> DbResult<VirtualKey> {
        sqlx::query_as::<_, VirtualKey>(
            r#"
            SELECT id, tenant_id, team_id, user_id, name, key_hash, key_prefix,
                   backend_id, allowed_models, rate_limit_rpm, token_limit_tpm,
                   budget_daily, budget_monthly, metadata, is_active, expires_at,
                   last_used_at, created_at, updated_at
            FROM virtual_keys
            WHERE key_hash = $1 AND is_active = true
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound("Virtual key".to_string()))
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<VirtualKey>> {
        let keys = sqlx::query_as::<_, VirtualKey>(
            r#"
            SELECT id, tenant_id, team_id, user_id, name, key_hash, key_prefix,
                   backend_id, allowed_models, rate_limit_rpm, token_limit_tpm,
                   budget_daily, budget_monthly, metadata, is_active, expires_at,
                   last_used_at, created_at, updated_at
            FROM virtual_keys
            WHERE tenant_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(keys)
    }

    pub async fn revoke(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query(
            "UPDATE virtual_keys SET is_active = false, updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch_used(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE virtual_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count_by_tenant(&self, tenant_id: Uuid) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM virtual_keys WHERE tenant_id = $1 AND is_active = true",
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }
}
