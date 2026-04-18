use dashmap::{DashMap, DashSet};
use uuid::Uuid;

use crate::error::PolicyError;

/// Per-tenant IP allowlist/denylist enforcer.
///
/// Supports both global rules (applied to all tenants) and per-tenant rules.
pub struct IpFilter {
    /// Global denylist (applied before tenant rules).
    global_denylist: DashSet<String>,
    /// Per-tenant denylists.
    tenant_denylists: DashMap<Uuid, DashSet<String>>,
    /// Per-tenant allowlists. If a tenant has an allowlist, only those IPs are allowed.
    tenant_allowlists: DashMap<Uuid, DashSet<String>>,
}

impl IpFilter {
    pub fn new() -> Self {
        Self {
            global_denylist: DashSet::new(),
            tenant_denylists: DashMap::new(),
            tenant_allowlists: DashMap::new(),
        }
    }

    // ── Global rules ───────────────────────────────────────────────────────

    pub fn global_deny(&self, ip: &str) {
        self.global_denylist.insert(ip.to_owned());
    }

    pub fn global_remove_deny(&self, ip: &str) {
        self.global_denylist.remove(ip);
    }

    // ── Per-tenant rules ───────────────────────────────────────────────────

    pub fn tenant_deny(&self, tenant_id: Uuid, ip: &str) {
        self.tenant_denylists
            .entry(tenant_id)
            .or_insert_with(DashSet::new)
            .insert(ip.to_owned());
    }

    pub fn tenant_allow(&self, tenant_id: Uuid, ip: &str) {
        self.tenant_allowlists
            .entry(tenant_id)
            .or_insert_with(DashSet::new)
            .insert(ip.to_owned());
    }

    pub fn tenant_remove_deny(&self, tenant_id: Uuid, ip: &str) {
        if let Some(list) = self.tenant_denylists.get(&tenant_id) {
            list.remove(ip);
        }
    }

    pub fn tenant_remove_allow(&self, tenant_id: Uuid, ip: &str) {
        if let Some(list) = self.tenant_allowlists.get(&tenant_id) {
            list.remove(ip);
        }
    }

    // ── Checking ───────────────────────────────────────────────────────────

    /// Check IP against global rules only (backward compatible).
    pub fn check(&self, ip: &str) -> Result<(), PolicyError> {
        if self.global_denylist.contains(ip) {
            return Err(PolicyError::IpBlocked);
        }
        Ok(())
    }

    /// Check IP against both global and tenant-specific rules.
    pub fn check_for_tenant(&self, ip: &str, tenant_id: Uuid) -> Result<(), PolicyError> {
        // 1. Global denylist
        if self.global_denylist.contains(ip) {
            return Err(PolicyError::IpBlocked);
        }

        // 2. Tenant denylist
        if let Some(denylist) = self.tenant_denylists.get(&tenant_id) {
            if denylist.contains(ip) {
                return Err(PolicyError::IpBlocked);
            }
        }

        // 3. Tenant allowlist (if set, only whitelisted IPs pass)
        if let Some(allowlist) = self.tenant_allowlists.get(&tenant_id) {
            if !allowlist.is_empty() && !allowlist.contains(ip) {
                return Err(PolicyError::IpBlocked);
            }
        }

        Ok(())
    }
}

impl Default for IpFilter {
    fn default() -> Self {
        Self::new()
    }
}
