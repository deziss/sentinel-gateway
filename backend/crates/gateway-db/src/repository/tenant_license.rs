use crate::{DbPool, error::DbResult};
use crate::models::tenant_license::{TenantLicense, CreateTenantLicense, UpdateTenantLicense};
use uuid::Uuid;

#[derive(Clone)]
pub struct TenantLicenseRepository {
    pool: DbPool,
}

impl TenantLicenseRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateTenantLicense) -> DbResult<TenantLicense> {
        let license = sqlx::query_as::<_, TenantLicense>(
            r#"
            INSERT INTO tenant_licenses (
                tenant_id, license_key, license_type, status, plan, entitlements, fingerprint, expires_at, offline_token
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#
        )
        .bind(input.tenant_id)
        .bind(input.license_key)
        .bind(input.license_type)
        .bind(input.status)
        .bind(input.plan)
        .bind(input.entitlements)
        .bind(input.fingerprint)
        .bind(input.expires_at)
        .bind(input.offline_token)
        .fetch_one(&self.pool)
        .await?;

        Ok(license)
    }

    pub async fn find_by_tenant_id(&self, tenant_id: Uuid) -> DbResult<Option<TenantLicense>> {
        let license = sqlx::query_as::<_, TenantLicense>(
            "SELECT * FROM tenant_licenses WHERE tenant_id = $1"
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(license)
    }

    pub async fn update(&self, tenant_id: Uuid, input: UpdateTenantLicense) -> DbResult<TenantLicense> {
        // `Separated::push` prefixes every call after the first with ", ", so we need
        // `push_bind_unseparated` when appending the value to the same column fragment
        // — otherwise we'd emit `status = , $1` instead of `status = $1`.
        let mut query_builder = sqlx::QueryBuilder::new("UPDATE tenant_licenses SET ");
        let mut separated = query_builder.separated(", ");

        if let Some(status) = input.status {
            separated.push("status = ");
            separated.push_bind_unseparated(status);
        }
        if let Some(plan) = input.plan {
            separated.push("plan = ");
            separated.push_bind_unseparated(plan);
        }
        if let Some(entitlements) = input.entitlements {
            separated.push("entitlements = ");
            separated.push_bind_unseparated(entitlements);
        }
        if let Some(fingerprint) = input.fingerprint {
            separated.push("fingerprint = ");
            separated.push_bind_unseparated(fingerprint);
        }
        if let Some(expires_at) = input.expires_at {
            separated.push("expires_at = ");
            separated.push_bind_unseparated(expires_at);
        }
        if let Some(last_validated_at) = input.last_validated_at {
            separated.push("last_validated_at = ");
            separated.push_bind_unseparated(last_validated_at);
        }
        if let Some(last_reported_at) = input.last_reported_at {
            separated.push("last_reported_at = ");
            separated.push_bind_unseparated(last_reported_at);
        }
        if let Some(offline_token) = input.offline_token {
            separated.push("offline_token = ");
            separated.push_bind_unseparated(offline_token);
        }

        separated.push("updated_at = CURRENT_TIMESTAMP");

        query_builder.push(" WHERE tenant_id = ");
        query_builder.push_bind(tenant_id);
        query_builder.push(" RETURNING *");

        let license = query_builder
            .build_query_as::<TenantLicense>()
            .fetch_one(&self.pool)
            .await?;

        Ok(license)
    }

    pub async fn delete(&self, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM tenant_licenses WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn list_for_heartbeat(&self) -> DbResult<Vec<TenantLicense>> {
        let licenses = sqlx::query_as::<_, TenantLicense>(
            r#"
            SELECT * FROM tenant_licenses 
            WHERE status = 'active' AND license_type = 'online'
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(licenses)
    }

    pub async fn list_for_usage_report(&self) -> DbResult<Vec<TenantLicense>> {
        // Find licenses that are active and online
        let licenses = sqlx::query_as::<_, TenantLicense>(
            r#"
            SELECT * FROM tenant_licenses 
            WHERE status = 'active' AND license_type = 'online'
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(licenses)
    }
}
