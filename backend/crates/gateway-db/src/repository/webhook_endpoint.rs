use crate::error::DbResult;
use crate::models::webhook_endpoint::{CreateWebhookEndpoint, WebhookEndpoint};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct WebhookEndpointRepository {
    pool: DbPool,
}

impl WebhookEndpointRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateWebhookEndpoint) -> DbResult<WebhookEndpoint> {
        let id = Uuid::new_v4();
        let wh = sqlx::query_as::<_, WebhookEndpoint>(
            r#"
            INSERT INTO webhook_endpoints (id, tenant_id, url, events, secret)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, tenant_id, url, events, secret, is_active, last_sent_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.url)
        .bind(&input.events)
        .bind(input.secret)
        .fetch_one(&self.pool)
        .await?;
        Ok(wh)
    }

    pub async fn list_active_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<WebhookEndpoint>> {
        let endpoints = sqlx::query_as::<_, WebhookEndpoint>(
            r#"
            SELECT id, tenant_id, url, events, secret, is_active, last_sent_at, created_at, updated_at
            FROM webhook_endpoints WHERE tenant_id = $1 AND is_active = true
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(endpoints)
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<WebhookEndpoint>> {
        let endpoints = sqlx::query_as::<_, WebhookEndpoint>(
            r#"
            SELECT id, tenant_id, url, events, secret, is_active, last_sent_at, created_at, updated_at
            FROM webhook_endpoints WHERE tenant_id = $1 ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(endpoints)
    }

    pub async fn find_by_id(&self, id: Uuid, tenant_id: Uuid) -> DbResult<WebhookEndpoint> {
        sqlx::query_as::<_, WebhookEndpoint>(
            r#"
            SELECT id, tenant_id, url, events, secret, is_active, last_sent_at, created_at, updated_at
            FROM webhook_endpoints WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::error::DbError::NotFound(format!("Webhook {id}")))
    }

    pub async fn delete(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM webhook_endpoints WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_active(&self, id: Uuid, tenant_id: Uuid, is_active: bool) -> DbResult<()> {
        sqlx::query(
            "UPDATE webhook_endpoints SET is_active = $3, updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(is_active)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch_sent(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE webhook_endpoints SET last_sent_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
