use crate::error::DbResult;
use crate::models::guardrail_rule::{CreateGuardrailRule, GuardrailRule, UpdateGuardrailRule};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct GuardrailRuleRepository {
    pool: DbPool,
}

impl GuardrailRuleRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateGuardrailRule) -> DbResult<GuardrailRule> {
        let rule = sqlx::query_as::<_, GuardrailRule>(
            r#"
            INSERT INTO guardrail_rules (
                tenant_id, name, kind, stage, mode, category, config, priority, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, tenant_id, name, kind, stage, mode, category, config,
                      priority, is_active, created_by, created_at, updated_at
            "#,
        )
        .bind(input.tenant_id)
        .bind(&input.name)
        .bind(&input.kind)
        .bind(&input.stage)
        .bind(&input.mode)
        .bind(&input.category)
        .bind(&input.config)
        .bind(input.priority)
        .bind(input.created_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(rule)
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<GuardrailRule>> {
        let rules = sqlx::query_as::<_, GuardrailRule>(
            r#"
            SELECT id, tenant_id, name, kind, stage, mode, category, config,
                   priority, is_active, created_by, created_at, updated_at
            FROM guardrail_rules
            WHERE tenant_id = $1
            ORDER BY priority, name
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rules)
    }

    /// List all active rules for a tenant, ordered by priority (lowest first).
    /// This is the hot path called at request time.
    pub async fn list_active(&self, tenant_id: Uuid) -> DbResult<Vec<GuardrailRule>> {
        let rules = sqlx::query_as::<_, GuardrailRule>(
            r#"
            SELECT id, tenant_id, name, kind, stage, mode, category, config,
                   priority, is_active, created_by, created_at, updated_at
            FROM guardrail_rules
            WHERE tenant_id = $1 AND is_active = true
            ORDER BY priority, name
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rules)
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> DbResult<Option<GuardrailRule>> {
        let rule = sqlx::query_as::<_, GuardrailRule>(
            r#"
            SELECT id, tenant_id, name, kind, stage, mode, category, config,
                   priority, is_active, created_by, created_at, updated_at
            FROM guardrail_rules
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(rule)
    }

    pub async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateGuardrailRule,
    ) -> DbResult<GuardrailRule> {
        let rule = sqlx::query_as::<_, GuardrailRule>(
            r#"
            UPDATE guardrail_rules
            SET
                kind      = COALESCE($3, kind),
                stage     = COALESCE($4, stage),
                mode      = COALESCE($5, mode),
                category  = COALESCE($6, category),
                config    = COALESCE($7, config),
                priority  = COALESCE($8, priority),
                is_active = COALESCE($9, is_active),
                updated_at = NOW()
            WHERE tenant_id = $1 AND id = $2
            RETURNING id, tenant_id, name, kind, stage, mode, category, config,
                      priority, is_active, created_by, created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(input.kind)
        .bind(input.stage)
        .bind(input.mode)
        .bind(input.category)
        .bind(input.config)
        .bind(input.priority)
        .bind(input.is_active)
        .fetch_one(&self.pool)
        .await?;
        Ok(rule)
    }

    pub async fn delete(&self, tenant_id: Uuid, id: Uuid) -> DbResult<()> {
        sqlx::query(r#"DELETE FROM guardrail_rules WHERE tenant_id = $1 AND id = $2"#)
            .bind(tenant_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
