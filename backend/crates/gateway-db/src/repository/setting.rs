use crate::error::DbResult;
use crate::models::setting::Setting;
use crate::pool::DbPool;
use std::collections::HashMap;
use uuid::Uuid;

pub struct SettingRepository {
    pool: DbPool,
}

impl SettingRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<Setting>> {
        let settings = sqlx::query_as::<_, Setting>(
            r#"
            SELECT id, tenant_id, key, value, encrypted, updated_at
            FROM settings WHERE tenant_id = $1 ORDER BY key
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(settings)
    }

    pub async fn get(&self, tenant_id: Uuid, key: &str) -> DbResult<Option<Setting>> {
        let setting = sqlx::query_as::<_, Setting>(
            r#"
            SELECT id, tenant_id, key, value, encrypted, updated_at
            FROM settings WHERE tenant_id = $1 AND key = $2
            "#,
        )
        .bind(tenant_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(setting)
    }

    pub async fn upsert(
        &self,
        tenant_id: Uuid,
        key: &str,
        value: &str,
        encrypted: bool,
    ) -> DbResult<Setting> {
        let setting = sqlx::query_as::<_, Setting>(
            r#"
            INSERT INTO settings (id, tenant_id, key, value, encrypted, updated_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())
            ON CONFLICT (tenant_id, key)
            DO UPDATE SET value = $3, encrypted = $4, updated_at = NOW()
            RETURNING id, tenant_id, key, value, encrypted, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(key)
        .bind(value)
        .bind(encrypted)
        .fetch_one(&self.pool)
        .await?;
        Ok(setting)
    }

    pub async fn delete(&self, tenant_id: Uuid, key: &str) -> DbResult<()> {
        sqlx::query("DELETE FROM settings WHERE tenant_id = $1 AND key = $2")
            .bind(tenant_id)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Return all settings for a tenant as a key-value map.
    pub async fn get_map(&self, tenant_id: Uuid) -> DbResult<HashMap<String, String>> {
        let settings = self.list_by_tenant(tenant_id).await?;
        let map = settings
            .into_iter()
            .map(|s| (s.key, s.value))
            .collect();
        Ok(map)
    }
}
