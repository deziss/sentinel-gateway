use crate::error::DbResult;
use crate::models::route::{CreateRoute, Route};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct RouteRepository {
    pool: DbPool,
}

impl RouteRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateRoute) -> DbResult<Route> {
        let id = Uuid::new_v4();
        let route = sqlx::query_as::<_, Route>(
            r#"
            INSERT INTO routes (id, tenant_id, name, protocol, path_pattern, backend_id, strip_prefix, rewrite_rules)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, tenant_id, name, protocol,
                      path_pattern, backend_id, strip_prefix, rewrite_rules, is_active,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.name)
        .bind(input.protocol)
        .bind(input.path_pattern)
        .bind(input.backend_id)
        .bind(input.strip_prefix)
        .bind(input.rewrite_rules)
        .fetch_one(&self.pool)
        .await?;
        Ok(route)
    }

    pub async fn list_active_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<Route>> {
        let routes = sqlx::query_as::<_, Route>(
            r#"
            SELECT id, tenant_id, name, protocol,
                   path_pattern, backend_id, strip_prefix, rewrite_rules, is_active,
                   created_at, updated_at
            FROM routes WHERE tenant_id = $1 AND is_active = true
            ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(routes)
    }

    pub async fn delete(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query(
            "UPDATE routes SET is_active = false, updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
