use crate::error::{DbError, DbResult};
use crate::models::api_key::{ApiKey, CreateApiKey};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct ApiKeyRepository {
    pool: DbPool,
}

impl ApiKeyRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateApiKey) -> DbResult<ApiKey> {
        let id = Uuid::new_v4();
        let key = sqlx::query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (id, tenant_id, user_id, key_hash, name, scopes,
                                  rate_limit_rpm, budget_daily, budget_monthly, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, tenant_id, user_id, key_hash, name, scopes,
                      rate_limit_rpm, budget_daily, budget_monthly, is_active,
                      expires_at, last_used_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.user_id)
        .bind(input.key_hash)
        .bind(input.name)
        .bind(&input.scopes)
        .bind(input.rate_limit_rpm)
        .bind(input.budget_daily)
        .bind(input.budget_monthly)
        .bind(input.expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(key)
    }

    pub async fn find_by_hash(&self, key_hash: &str) -> DbResult<ApiKey> {
        sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, tenant_id, user_id, key_hash, name, scopes,
                   rate_limit_rpm, budget_daily, budget_monthly, is_active,
                   expires_at, last_used_at, created_at, updated_at
            FROM api_keys WHERE key_hash = $1 AND is_active = true
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound("API key".to_string()))
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<ApiKey>> {
        let keys = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, tenant_id, user_id, key_hash, name, scopes,
                   rate_limit_rpm, budget_daily, budget_monthly, is_active,
                   expires_at, last_used_at, created_at, updated_at
            FROM api_keys WHERE tenant_id = $1 ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(keys)
    }

    pub async fn count_by_tenant(&self, tenant_id: Uuid) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM api_keys WHERE tenant_id = $1 AND is_active = true",
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn revoke(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query(
            "UPDATE api_keys SET is_active = false, updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch_used(&self, id: Uuid) -> DbResult<()> {
        sqlx::query(
            "UPDATE api_keys SET last_used_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
