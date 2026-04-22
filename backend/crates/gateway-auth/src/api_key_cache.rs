use crate::roles::Role;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Cached API key entry: the key ID, owner info, and when it was cached.
#[derive(Debug, Clone)]
pub struct CachedApiKey {
    pub key_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub scopes: Vec<String>,
    pub is_active: bool,
    pub rate_limit_rpm: Option<i32>,
    pub budget_daily: Option<f64>,
    pub budget_monthly: Option<f64>,
    cached_at: Instant,
}

/// In-memory cache for API key lookups, keyed by SHA-256 hash of the key.
pub struct ApiKeyCache {
    cache: DashMap<String, CachedApiKey>,
    ttl: Duration,
}

impl ApiKeyCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            ttl,
        }
    }

    /// Look up a cached entry by key hash. Returns `None` if missing or expired.
    pub fn get(&self, key_hash: &str) -> Option<CachedApiKey> {
        match self.cache.get(key_hash) {
            Some(entry) if entry.cached_at.elapsed() < self.ttl => Some(entry.clone()),
            Some(_) => {
                // Expired — remove lazily
                self.cache.remove(key_hash);
                None
            }
            None => None,
        }
    }

    /// Insert or update a cache entry.
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &self,
        key_hash: String,
        key_id: Uuid,
        tenant_id: Uuid,
        user_id: Uuid,
        role: Role,
        scopes: Vec<String>,
        is_active: bool,
        rate_limit_rpm: Option<i32>,
        budget_daily: Option<f64>,
        budget_monthly: Option<f64>,
    ) {
        self.cache.insert(key_hash, CachedApiKey {
            key_id,
            tenant_id,
            user_id,
            role,
            scopes,
            is_active,
            rate_limit_rpm,
            budget_daily,
            budget_monthly,
            cached_at: Instant::now(),
        });
    }

    /// Remove a specific entry (e.g., after key revocation).
    pub fn invalidate(&self, key_hash: &str) {
        self.cache.remove(key_hash);
    }

    /// Remove all expired entries. Call periodically from a background task.
    pub fn cleanup(&self) {
        let ttl = self.ttl;
        self.cache.retain(|_, entry| entry.cached_at.elapsed() < ttl);
    }
}
