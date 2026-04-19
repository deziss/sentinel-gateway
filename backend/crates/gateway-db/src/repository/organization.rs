use crate::error::{DbError, DbResult};
use crate::models::organization::{CreateOrganization, Organization};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct OrganizationRepository {
    pool: DbPool,
}

impl OrganizationRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateOrganization) -> DbResult<Organization> {
        let id = Uuid::new_v4();
        let org = sqlx::query_as::<_, Organization>(
            r#"
            INSERT INTO organizations (id, slug, name, plan, metadata)
            VALUES ($1, $2, $3, COALESCE($4, 'free'), COALESCE($5, '{}'::jsonb))
            RETURNING id, slug, name, plan, metadata, is_active, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&input.slug)
        .bind(&input.name)
        .bind(&input.plan)
        .bind(&input.metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(org)
    }

    pub async fn find_by_id(&self, id: Uuid) -> DbResult<Organization> {
        sqlx::query_as::<_, Organization>(
            r#"SELECT id, slug, name, plan, metadata, is_active, created_at, updated_at
               FROM organizations WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Organization {id}")))
    }

    pub async fn find_by_slug(&self, slug: &str) -> DbResult<Organization> {
        sqlx::query_as::<_, Organization>(
            r#"SELECT id, slug, name, plan, metadata, is_active, created_at, updated_at
               FROM organizations WHERE slug = $1"#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Organization {slug}")))
    }

    pub async fn list(&self) -> DbResult<Vec<Organization>> {
        let rows = sqlx::query_as::<_, Organization>(
            r#"SELECT id, slug, name, plan, metadata, is_active, created_at, updated_at
               FROM organizations WHERE is_active = true ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn assign_tenant(&self, tenant_id: Uuid, organization_id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE tenants SET organization_id = $1, updated_at = NOW() WHERE id = $2")
            .bind(organization_id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_tenants(&self, organization_id: Uuid) -> DbResult<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM tenants WHERE organization_id = $1 ORDER BY created_at DESC",
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE organizations SET is_active = false, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
