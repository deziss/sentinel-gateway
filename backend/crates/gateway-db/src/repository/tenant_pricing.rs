use crate::error::DbResult;
use crate::models::tenant_pricing::{TenantPricing, UpsertTenantPricing};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct TenantPricingRepository {
    pool: DbPool,
}

impl TenantPricingRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, input: UpsertTenantPricing) -> DbResult<TenantPricing> {
        let pricing = sqlx::query_as::<_, TenantPricing>(
            r#"
            INSERT INTO tenant_pricing
                (id, tenant_id, model, input_per_1m, output_per_1m, markup_multiplier)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)
            ON CONFLICT (tenant_id, model) DO UPDATE SET
                input_per_1m = $3,
                output_per_1m = $4,
                markup_multiplier = $5,
                is_active = true,
                updated_at = NOW()
            RETURNING id, tenant_id, model, input_per_1m, output_per_1m,
                      markup_multiplier, is_active, created_at, updated_at
            "#,
        )
        .bind(input.tenant_id)
        .bind(&input.model)
        .bind(input.input_per_1m)
        .bind(input.output_per_1m)
        .bind(input.markup_multiplier)
        .fetch_one(&self.pool)
        .await?;
        Ok(pricing)
    }

    pub async fn get_for_model(&self, tenant_id: Uuid, model: &str) -> DbResult<Option<TenantPricing>> {
        let pricing = sqlx::query_as::<_, TenantPricing>(
            r#"
            SELECT id, tenant_id, model, input_per_1m, output_per_1m,
                   markup_multiplier, is_active, created_at, updated_at
            FROM tenant_pricing
            WHERE tenant_id = $1 AND model = $2 AND is_active = true
            "#,
        )
        .bind(tenant_id)
        .bind(model)
        .fetch_optional(&self.pool)
        .await?;
        Ok(pricing)
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<TenantPricing>> {
        let rows = sqlx::query_as::<_, TenantPricing>(
            r#"
            SELECT id, tenant_id, model, input_per_1m, output_per_1m,
                   markup_multiplier, is_active, created_at, updated_at
            FROM tenant_pricing
            WHERE tenant_id = $1 AND is_active = true
            ORDER BY model
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete(&self, tenant_id: Uuid, model: &str) -> DbResult<()> {
        sqlx::query(
            "UPDATE tenant_pricing SET is_active = false, updated_at = NOW()
             WHERE tenant_id = $1 AND model = $2",
        )
        .bind(tenant_id)
        .bind(model)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
