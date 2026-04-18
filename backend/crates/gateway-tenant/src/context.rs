use gateway_db::models::Tenant;
use uuid::Uuid;

/// Tenant context injected into every request
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant: Tenant,
    pub is_saas_mode: bool,
}

impl TenantContext {
    pub fn new(tenant: Tenant, is_saas_mode: bool) -> Self {
        Self { tenant, is_saas_mode }
    }

    pub fn id(&self) -> Uuid {
        self.tenant.id
    }

    pub fn plan(&self) -> &str {
        &self.tenant.plan
    }

    pub fn can_add_user(&self, current_count: i32) -> bool {
        current_count < self.tenant.max_users
    }

    pub fn can_add_api_key(&self, current_count: i32) -> bool {
        current_count < self.tenant.max_api_keys
    }

    pub fn can_add_backend(&self, current_count: i32) -> bool {
        current_count < self.tenant.max_backends
    }
}
