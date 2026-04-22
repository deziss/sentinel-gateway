use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use tracing::{error, warn};
use fred::prelude::*;
use fred::types::Expiration;
use fred::interfaces::KeysInterface;

use crate::features::{FeatureFlags, Plan};
use crate::error::LicenseError;
use gateway_db::repository::TenantLicenseRepository;

#[cfg(feature = "saas")]
use crate::client::{LicenciaClient, ValidateRequest};

/// Service for resolving and caching per-tenant feature flags.
/// Uses a 3-tier cache hierarchy:
/// 1. L1: In-memory DashMap (fastest, per-replica)
/// 2. L2: Redis (consistent across replicas)
/// 3. L3: PostgreSQL (source of truth)
pub struct TenantLicenseService {
    repo: Arc<TenantLicenseRepository>,
    #[cfg(feature = "saas")]
    licencia_client: Option<Arc<LicenciaClient>>,
    redis: Option<RedisClient>,
    
    // L1 Cache: tenant_id -> (FeatureFlags, expiration)
    l1_cache: DashMap<Uuid, (FeatureFlags, DateTime<Utc>)>,
    
    l1_ttl: Duration,
    l2_ttl_secs: i64,
}

impl TenantLicenseService {
    pub fn new(
        repo: Arc<TenantLicenseRepository>,
        #[cfg(feature = "saas")]
        licencia_client: Option<Arc<LicenciaClient>>,
        redis: Option<RedisClient>,
    ) -> Self {
        Self {
            repo,
            #[cfg(feature = "saas")]
            licencia_client,
            redis,
            l1_cache: DashMap::new(),
            l1_ttl: Duration::minutes(5),
            l2_ttl_secs: 3600, // 1 hour
        }
    }

    /// Resolves feature flags for a tenant.
    pub async fn resolve(&self, tenant_id: Uuid) -> FeatureFlags {
        // 1. Try L1 Cache
        if let Some(entry) = self.l1_cache.get(&tenant_id) {
            let (flags, expires_at) = entry.value();
            if *expires_at > Utc::now() {
                return flags.clone();
            }
        }

        // 2. Try L2 Cache (Redis)
        if let Some(ref redis) = self.redis {
            let key = format!("license:tenant:{}", tenant_id);
            let cached: Option<String> = redis.get(&key).await.ok();
            if let Some(cached) = cached {
                if let Ok(flags) = serde_json::from_str::<FeatureFlags>(&cached) {
                    // Backfill L1
                    self.l1_cache.insert(tenant_id, (flags.clone(), Utc::now() + self.l1_ttl));
                    return flags;
                }
            }
        }

        // 3. Try L3 (DB)
        let flags = match self.repo.find_by_tenant_id(tenant_id).await {
            Ok(Some(license)) => {
                if license.status != "active" {
                    FeatureFlags::for_plan(Plan::Community)
                } else {
                    // Parse entitlements from JSONB
                    serde_json::from_value(license.entitlements)
                        .unwrap_or_else(|_| {
                            warn!("Failed to parse entitlements for tenant {}, falling back to plan defaults", tenant_id);
                            FeatureFlags::for_plan(Plan::from_str(&license.plan))
                        })
                }
            }
            Ok(None) => FeatureFlags::for_plan(Plan::Community),
            Err(e) => {
                error!("Database error resolving license for tenant {}: {}", tenant_id, e);
                FeatureFlags::for_plan(Plan::Community)
            }
        };

        // Update Caches
        self.update_caches(tenant_id, flags.clone()).await;
        flags
    }

    async fn update_caches(&self, tenant_id: Uuid, flags: FeatureFlags) {
        // Update L1
        self.l1_cache.insert(tenant_id, (flags.clone(), Utc::now() + self.l1_ttl));

        // Update L2 (Redis)
        if let Some(ref redis) = self.redis {
            let key = format!("license:tenant:{}", tenant_id);
            if let Ok(serialized) = serde_json::to_string(&flags) {
                let _: Result<(), _> = redis.set(key, serialized, Some(Expiration::EX(self.l2_ttl_secs)), None, false).await;
            }
        }
    }

    /// Forces a refresh of the license for a tenant, hitting Licencia if online.
    pub async fn refresh(&self, tenant_id: Uuid) -> Result<FeatureFlags, LicenseError> {
        let license = self.repo.find_by_tenant_id(tenant_id).await
            .map_err(|e| LicenseError::Database(e.to_string()))?
            .ok_or_else(|| LicenseError::NotFound(format!("No license found for tenant {}", tenant_id)))?;

        #[cfg(feature = "saas")]
        if license.license_type == "online" {
            if let Some(ref client) = self.licencia_client {
                let req = ValidateRequest {
                    key: license.license_key.clone(),
                    hardware_id: license.fingerprint.clone(),
                    device_name: None,
                    os: None,
                    app_version: None,
                };

                match client.validate(req).await {
                    Ok(resp) => {
                        // Use entitlements from response or fallback to plan
                        let flags = if let Some(ent) = resp.entitlements {
                            serde_json::from_value(ent).unwrap_or_else(|_| FeatureFlags::for_plan(Plan::from_str(&license.plan)))
                        } else {
                            FeatureFlags::for_plan(Plan::from_str(&license.plan))
                        };
                        
                        // Update DB. `serde_json::to_value(&flags)` is infallible for our
                        // `FeatureFlags` shape (no non-string map keys, no non-finite floats),
                        // so `map_err` over `unwrap` just to surface a plausible error if the
                        // shape ever changes.
                        let entitlements_json = serde_json::to_value(&flags)
                            .map_err(|e| LicenseError::Internal(format!("serialize flags: {e}")))?;
                        let update = gateway_db::models::tenant_license::UpdateTenantLicense {
                            status: Some("active".to_string()),
                            plan: Some(license.plan.clone()),
                            entitlements: Some(entitlements_json),
                            fingerprint: license.fingerprint.clone(),
                            expires_at: Some(resp.expires_at.as_ref().and_then(|s| s.parse::<DateTime<Utc>>().ok())),
                            last_validated_at: Some(Utc::now()),
                            last_reported_at: None,
                            offline_token: None,
                        };
                        
                        self.repo.update(tenant_id, update).await
                            .map_err(|e| LicenseError::Database(e.to_string()))?;

                        self.update_caches(tenant_id, flags.clone()).await;
                        return Ok(flags);
                    }
                    Err(e) => {
                        warn!("Licencia validation failed during refresh for tenant {}: {}", tenant_id, e);
                    }
                }
            }
        }

        // For offline or failed online refresh, resolve from DB
        Ok(self.resolve(tenant_id).await)
    }

    /// Invalidate L1 and L2 cache for a tenant.
    pub async fn invalidate(&self, tenant_id: Uuid) {
        self.l1_cache.remove(&tenant_id);
        if let Some(ref redis) = self.redis {
            let key = format!("license:tenant:{}", tenant_id);
            let _: Result<(), _> = redis.del(key).await;
        }
    }
}
