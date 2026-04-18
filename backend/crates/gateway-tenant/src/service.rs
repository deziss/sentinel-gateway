use dashmap::DashMap;
use gateway_db::{models::Tenant, repository::TenantRepository};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::TenantError;

pub struct TenantService {
    repo: Arc<TenantRepository>,
    /// Cache: slug/id → Tenant  (simple in-memory TTL-less cache)
    cache: DashMap<String, Tenant>,
    default_tenant_id: Option<Uuid>,
}

impl TenantService {
    pub fn new(repo: Arc<TenantRepository>, default_tenant_id: Option<Uuid>) -> Self {
        Self {
            repo,
            cache: DashMap::new(),
            default_tenant_id,
        }
    }

    /// Resolve tenant from optional header, JWT claim, api_key context, or subdomain.
    /// Priority: Header > JWT claim > API Key > Subdomain > Default
    pub async fn resolve(
        &self,
        tenant_id_header: Option<&str>,
        jwt_tenant_id: Option<Uuid>,
        api_key_tenant_id: Option<Uuid>,
        host: Option<&str>,
    ) -> Result<Tenant, TenantError> {
        // 1. X-Tenant-ID header (UUID or slug) - Explicit Override
        if let Some(id_str) = tenant_id_header {
            if let Ok(id) = Uuid::parse_str(id_str) {
                return self.find_by_id(id).await;
            }
            if let Ok(t) = self.find_by_slug(id_str).await {
                return Ok(t);
            }
        }

        // 2. JWT claim tid - from authenticated user context
        if let Some(id) = jwt_tenant_id {
            return self.find_by_id(id).await;
        }

        // 3. API Key Context - if we already identified a key, it's a strong identifier
        if let Some(id) = api_key_tenant_id {
            return self.find_by_id(id).await;
        }

        // 4. Subdomain: extract first segment from host
        if let Some(host) = host {
            let subdomain = host.split('.').next().unwrap_or("");
            if !subdomain.is_empty() && subdomain != "www" && subdomain != "api" && subdomain != "localhost" {
                if let Ok(t) = self.find_by_slug(subdomain).await {
                    return Ok(t);
                }
            }
        }

        // 4. Default tenant (SaaS / Global mode)
        if let Some(default_id) = self.default_tenant_id {
            return self.find_by_id(default_id).await;
        }

        Err(TenantError::NotFound)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Tenant, TenantError> {
        let key = id.to_string();
        if let Some(t) = self.cache.get(&key) {
            return Ok(t.clone());
        }
        let tenant = self
            .repo
            .find_by_id(id)
            .await
            .map_err(|_| TenantError::NotFound)?;
        self.cache.insert(key, tenant.clone());
        Ok(tenant)
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Tenant, TenantError> {
        if let Some(t) = self.cache.get(slug) {
            return Ok(t.clone());
        }
        let tenant = self
            .repo
            .find_by_slug(slug)
            .await
            .map_err(|_| TenantError::NotFound)?;
        self.cache.insert(slug.to_owned(), tenant.clone());
        self.cache.insert(tenant.id.to_string(), tenant.clone());
        Ok(tenant)
    }

    pub fn invalidate_cache(&self, tenant_id: Uuid) {
        self.cache.remove(&tenant_id.to_string());
    }
}
