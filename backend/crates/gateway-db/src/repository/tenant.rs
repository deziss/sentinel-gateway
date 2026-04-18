use crate::error::{DbError, DbResult};
use crate::models::tenant::{CreateTenant, Tenant, UpdateTenant};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct TenantRepository {
    pool: DbPool,
}

impl TenantRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateTenant) -> DbResult<Tenant> {
        let id = Uuid::new_v4();
        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            INSERT INTO tenants (id, name, slug, plan, settings, max_users, max_api_keys, max_backends)
            VALUES ($1, $2, $3, $4, '{}', $5, $6, $7)
            RETURNING id, name, slug, plan, settings, license_key, is_active,
                      max_users, max_api_keys, max_backends, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.name)
        .bind(input.slug)
        .bind(input.plan)
        .bind(input.max_users)
        .bind(input.max_api_keys)
        .bind(input.max_backends)
        .fetch_one(&self.pool)
        .await?;
        Ok(tenant)
    }

    pub async fn find_by_id(&self, id: Uuid) -> DbResult<Tenant> {
        sqlx::query_as::<_, Tenant>(
            r#"
            SELECT id, name, slug, plan, settings, license_key, is_active,
                   max_users, max_api_keys, max_backends, created_at, updated_at
            FROM tenants WHERE id = $1 AND is_active = true
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Tenant {id}")))
    }

    pub async fn find_by_slug(&self, slug: &str) -> DbResult<Tenant> {
        sqlx::query_as::<_, Tenant>(
            r#"
            SELECT id, name, slug, plan, settings, license_key, is_active,
                   max_users, max_api_keys, max_backends, created_at, updated_at
            FROM tenants WHERE slug = $1 AND is_active = true
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Tenant slug={slug}")))
    }

    pub async fn list(&self) -> DbResult<Vec<Tenant>> {
        let tenants = sqlx::query_as::<_, Tenant>(
            r#"
            SELECT id, name, slug, plan, settings, license_key, is_active,
                   max_users, max_api_keys, max_backends, created_at, updated_at
            FROM tenants ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(tenants)
    }

    pub async fn update(&self, id: Uuid, input: UpdateTenant) -> DbResult<Tenant> {
        // Build dynamic SET clause from non-None fields
        let mut sets = vec!["updated_at = NOW()".to_string()];
        let mut bind_idx = 2u32; // $1 is id

        macro_rules! push_set {
            ($field:ident, $col:expr) => {
                if input.$field.is_some() {
                    sets.push(format!("{} = ${bind_idx}", $col));
                    bind_idx += 1;
                }
            };
        }

        push_set!(name, "name");
        push_set!(plan, "plan");
        push_set!(settings, "settings");
        push_set!(max_users, "max_users");
        push_set!(max_api_keys, "max_api_keys");
        push_set!(max_backends, "max_backends");
        push_set!(is_active, "is_active");
        push_set!(license_key, "license_key");

        let sql = format!(
            "UPDATE tenants SET {} WHERE id = $1 RETURNING id, name, slug, plan, settings, license_key, is_active, max_users, max_api_keys, max_backends, created_at, updated_at",
            sets.join(", ")
        );

        let mut query = sqlx::query_as::<_, Tenant>(&sql).bind(id);

        if let Some(ref v) = input.name { query = query.bind(v); }
        if let Some(ref v) = input.plan { query = query.bind(v); }
        if let Some(ref v) = input.settings { query = query.bind(v); }
        if let Some(v) = input.max_users { query = query.bind(v); }
        if let Some(v) = input.max_api_keys { query = query.bind(v); }
        if let Some(v) = input.max_backends { query = query.bind(v); }
        if let Some(v) = input.is_active { query = query.bind(v); }
        if let Some(ref v) = input.license_key { query = query.bind(v); }

        let tenant = query
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| DbError::NotFound(format!("Tenant {id}")))?;
        Ok(tenant)
    }

    pub async fn count_active(&self) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tenants WHERE is_active = true",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE tenants SET is_active = false WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
