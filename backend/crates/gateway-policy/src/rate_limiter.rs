use dashmap::DashMap;
use fred::prelude::*;
use fred::interfaces::LuaInterface;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::error::PolicyError;

/// Rate limit key variants — determines what entity is being limited.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum RateLimitKey {
    ApiKey(Uuid),
    User(Uuid),
    Tenant(Uuid),
    Ip(String),
    Model(String),
    Lockout(String),
    /// Composite key: e.g., rate limit a specific model per user.
    Composite(String),
}

impl RateLimitKey {
    /// Create a per-user per-model composite key.
    pub fn user_model(user_id: Uuid, model: &str) -> Self {
        Self::Composite(format!("rl:usr:{user_id}:mdl:{model}"))
    }

    /// Create a per-tenant per-model composite key.
    pub fn tenant_model(tenant_id: Uuid, model: &str) -> Self {
        Self::Composite(format!("rl:ten:{tenant_id}:mdl:{model}"))
    }

    /// Create a **tokens-per-minute** key for an API key.
    pub fn tokens_api_key(api_key_id: Uuid) -> Self {
        Self::Composite(format!("rl:tok:key:{api_key_id}"))
    }

    /// Create a **tokens-per-minute** key for a tenant.
    pub fn tokens_tenant(tenant_id: Uuid) -> Self {
        Self::Composite(format!("rl:tok:ten:{tenant_id}"))
    }

    /// Create a **tokens-per-minute** key for a tenant+model combination.
    pub fn tokens_tenant_model(tenant_id: Uuid, model: &str) -> Self {
        Self::Composite(format!("rl:tok:ten:{tenant_id}:mdl:{model}"))
    }
}

impl std::fmt::Display for RateLimitKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitKey::ApiKey(id) => write!(f, "rl:key:{id}"),
            RateLimitKey::User(id) => write!(f, "rl:usr:{id}"),
            RateLimitKey::Tenant(id) => write!(f, "rl:ten:{id}"),
            RateLimitKey::Ip(ip) => write!(f, "rl:ip:{ip}"),
            RateLimitKey::Model(m) => write!(f, "rl:mdl:{m}"),
            RateLimitKey::Lockout(id) => write!(f, "rl:lkt:{id}"),
            RateLimitKey::Composite(key) => write!(f, "{key}"),
        }
    }
}

/// Result of a rate limit check.
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Total capacity (requests per window).
    pub limit: u32,
    /// Remaining requests in the current window.
    pub remaining: u32,
    /// Seconds until the window resets.
    pub reset_after_secs: u64,
    /// Seconds to wait before retrying (0 if allowed).
    pub retry_after_secs: u64,
}

// ── Token Bucket (in-memory) ───────────────────────────────────────────────

pub struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    capacity: f64,
    refill_rate: f64,
}

impl TokenBucket {
    fn new(rpm: u32) -> Self {
        let capacity = rpm as f64;
        Self {
            tokens: capacity,
            last_refill: Instant::now(),
            capacity,
            refill_rate: capacity / 60.0,
        }
    }

    fn try_consume(&mut self) -> RateLimitResult {
        self.try_consume_n(1.0)
    }

    pub(crate) fn try_consume_n(&mut self, units: f64) -> RateLimitResult {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;

        if self.tokens >= units {
            self.tokens -= units;
            RateLimitResult {
                allowed: true,
                limit: self.capacity as u32,
                remaining: self.tokens as u32,
                reset_after_secs: 60,
                retry_after_secs: 0,
            }
        } else {
            let retry = if self.refill_rate > 0.0 {
                ((units - self.tokens) / self.refill_rate).ceil() as u64
            } else {
                3600
            };
            RateLimitResult {
                allowed: false,
                limit: self.capacity as u32,
                remaining: self.tokens.max(0.0) as u32,
                reset_after_secs: 60,
                retry_after_secs: retry,
            }
        }
    }
}

// ── Sliding Window (in-memory) ─────────────────────────────────────────────

pub struct SlidingWindow {
    timestamps: VecDeque<Instant>,
    window: Duration,
    limit: u32,
}

impl SlidingWindow {
    fn new(limit: u32, window_secs: u64) -> Self {
        Self {
            timestamps: VecDeque::new(),
            window: Duration::from_secs(window_secs),
            limit,
        }
    }

    fn try_consume(&mut self) -> RateLimitResult {
        self.try_consume_n(1)
    }

    pub(crate) fn try_consume_n(&mut self, units: usize) -> RateLimitResult {
        let now = Instant::now();
        let cutoff = now - self.window;

        while self.timestamps.front().is_some_and(|t| *t < cutoff) {
            self.timestamps.pop_front();
        }

        let count = self.timestamps.len() as u32;
        let needed = units as u32;
        if count + needed <= self.limit {
            for _ in 0..units {
                self.timestamps.push_back(now);
            }
            RateLimitResult {
                allowed: true,
                limit: self.limit,
                remaining: self.limit - (count + needed),
                reset_after_secs: self.window.as_secs(),
                retry_after_secs: 0,
            }
        } else {
            let oldest = self.timestamps.front().map(|t| {
                let elapsed = now.duration_since(*t);
                self.window.as_secs().saturating_sub(elapsed.as_secs())
            }).unwrap_or(1);
            RateLimitResult {
                allowed: false,
                limit: self.limit,
                remaining: self.limit.saturating_sub(count),
                reset_after_secs: self.window.as_secs(),
                retry_after_secs: oldest.max(1),
            }
        }
    }
}

// ── Algorithm enum ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitAlgorithm {
    TokenBucket,
    SlidingWindow,
}

// ── Bucket entry ──────────────────────────────────────────────────────────

pub enum BucketEntry {
    Token(TokenBucket),
    Sliding(SlidingWindow),
}

// ── Pre-loaded Redis script SHAs ──────────────────────────────────────────

/// Lua script body for RPM (requests-per-minute) checking.
/// KEYS[1] = rate-limit key, ARGV[1] = limit (rpm), ARGV[2] = window TTL (60s)
const LUA_CHECK: &str = r"
    local key = KEYS[1]
    local limit = tonumber(ARGV[1])
    local ttl = tonumber(ARGV[2])
    local current = redis.call('INCR', key)
    if current == 1 then
        redis.call('EXPIRE', key, ttl)
    end
    local remaining = limit - current
    if remaining < 0 then remaining = 0 end
    if current > limit then
        return {0, remaining, ttl}
    else
        return {1, remaining, ttl}
    end
";

/// Lua script body for token/unit consumption.
/// KEYS[1] = key, ARGV[1] = limit, ARGV[2] = units, ARGV[3] = window TTL
const LUA_CONSUME: &str = r"
    local key = KEYS[1]
    local limit = tonumber(ARGV[1])
    local units = tonumber(ARGV[2])
    local ttl = tonumber(ARGV[3])
    local current = redis.call('INCRBY', key, units)
    if current == units then
        redis.call('EXPIRE', key, ttl)
    end
    local remaining = limit - current
    if remaining < 0 then remaining = 0 end
    if current > limit then
        return {0, remaining, ttl}
    else
        return {1, remaining, ttl}
    end
";

/// Pre-loaded SHA1 digests for the Lua scripts.
/// Populated at startup by [`RateLimiter::preload_scripts`].
#[derive(Debug, Clone, Default)]
pub struct ScriptShas {
    pub check: Option<String>,
    pub consume: Option<String>,
}

// ── Main Rate Limiter ──────────────────────────────────────────────────────

/// A dual-backend rate limiter (in-memory or Redis).
pub enum RateLimiter {
    InMemory {
        buckets: DashMap<String, BucketEntry>,
        default_algorithm: RateLimitAlgorithm,
    },
    Redis {
        client: RedisClient,
        /// SHA1s of pre-loaded Lua scripts; populated by `preload_scripts()`.
        shas: std::sync::Arc<tokio::sync::RwLock<ScriptShas>>,
    },
}

impl RateLimiter {
    pub fn new_in_memory() -> Self {
        Self::InMemory {
            buckets: DashMap::new(),
            default_algorithm: RateLimitAlgorithm::TokenBucket,
        }
    }

    pub fn new_in_memory_sliding() -> Self {
        Self::InMemory {
            buckets: DashMap::new(),
            default_algorithm: RateLimitAlgorithm::SlidingWindow,
        }
    }

    pub fn new_redis(client: RedisClient) -> Self {
        Self::Redis {
            client,
            shas: std::sync::Arc::new(tokio::sync::RwLock::new(ScriptShas::default())),
        }
    }

    /// Pre-load Lua scripts into Redis at startup.
    ///
    /// Must be called once after connecting to Redis. Subsequent `check` /
    /// `consume` calls will use EVALSHA instead of sending the full script body,
    /// reducing per-call bandwidth by ~80%.
    ///
    /// Falls back gracefully: if pre-load fails, calls use inline `EVAL`.
    pub async fn preload_scripts(&self) {
        if let Self::Redis { client, shas } = self {
            let check_sha: Result<String, _> = client.script_load(LUA_CHECK).await;
            let consume_sha: Result<String, _> = client.script_load(LUA_CONSUME).await;

            let mut guard = shas.write().await;
            match check_sha {
                Ok(sha) => {
                    tracing::info!(sha = %sha, "Redis LUA_CHECK pre-loaded");
                    guard.check = Some(sha);
                }
                Err(e) => tracing::warn!("Failed to pre-load LUA_CHECK: {e}; will use inline EVAL"),
            }
            match consume_sha {
                Ok(sha) => {
                    tracing::info!(sha = %sha, "Redis LUA_CONSUME pre-loaded");
                    guard.consume = Some(sha);
                }
                Err(e) => tracing::warn!("Failed to pre-load LUA_CONSUME: {e}; will use inline EVAL"),
            }
        }
    }

    /// Access the internal Redis client if this is a Redis-backed limiter.
    pub fn redis_client(&self) -> Option<&RedisClient> {
        match self {
            Self::Redis { client, .. } => Some(client),
            _ => None,
        }
    }

    // ── Internal Redis call helpers ────────────────────────────────────────

    /// Execute the rate-check script, using EVALSHA if available, EVAL otherwise.
    async fn redis_check(
        client: &RedisClient,
        shas: &tokio::sync::RwLock<ScriptShas>,
        key: String,
        rpm: u32,
    ) -> Result<Vec<i64>, fred::error::RedisError> {
        let sha = shas.read().await.check.clone();
        let args = vec![rpm as i64, 60i64];
        if let Some(sha) = sha {
            let res: Result<Vec<i64>, _> = client.evalsha(&sha, vec![key.clone()], args.clone()).await;
            if res.is_ok() {
                return res;
            }
            // NOSCRIPT — fall through to inline EVAL
        }
        client.eval(LUA_CHECK, vec![key], args).await
    }

    /// Execute the consume script, using EVALSHA if available, EVAL otherwise.
    async fn redis_consume(
        client: &RedisClient,
        shas: &tokio::sync::RwLock<ScriptShas>,
        key: String,
        limit: u32,
        units: u64,
    ) -> Result<Vec<i64>, fred::error::RedisError> {
        let sha = shas.read().await.consume.clone();
        let args = vec![limit as i64, units as i64, 60i64];
        if let Some(sha) = sha {
            let res: Result<Vec<i64>, _> = client.evalsha(&sha, vec![key.clone()], args.clone()).await;
            if res.is_ok() {
                return res;
            }
        }
        client.eval(LUA_CONSUME, vec![key], args).await
    }

    // ── Public API ─────────────────────────────────────────────────────────

    /// Check + consume one token for the given key at the given RPM limit.
    pub async fn check(&self, key: &RateLimitKey, rpm: u32) -> Result<(), PolicyError> {
        let result = self.check_detailed(key, rpm).await?;
        if result.allowed {
            Ok(())
        } else {
            Err(PolicyError::RateLimited {
                retry_after: result.retry_after_secs,
                limit: result.limit,
                remaining: result.remaining,
            })
        }
    }

    /// Consume N units from the bucket.
    pub async fn consume(&self, key: &RateLimitKey, limit: u32, units: u64) -> Result<RateLimitResult, PolicyError> {
        match self {
            Self::InMemory { buckets, default_algorithm } => {
                let map_key = key.to_string();
                let mut entry = buckets.entry(map_key).or_insert_with(|| {
                    match default_algorithm {
                        RateLimitAlgorithm::TokenBucket => BucketEntry::Token(TokenBucket::new(limit)),
                        RateLimitAlgorithm::SlidingWindow => BucketEntry::Sliding(SlidingWindow::new(limit, 60)),
                    }
                });
                let result = match entry.value_mut() {
                    BucketEntry::Token(b) => b.try_consume_n(units as f64),
                    BucketEntry::Sliding(w) => w.try_consume_n(units as usize),
                };
                Ok(result)
            }
            Self::Redis { client, shas } => {
                let redis_key = format!("{key}:tokens");
                let result = Self::redis_consume(client, shas, redis_key, limit, units)
                    .await
                    .map_err(|e| PolicyError::Internal(e.to_string()))?;

                let allowed = result.first().copied().unwrap_or(0) == 1;
                let remaining = result.get(1).copied().unwrap_or(0) as u32;
                Ok(RateLimitResult {
                    allowed,
                    limit,
                    remaining,
                    reset_after_secs: 60,
                    retry_after_secs: if allowed { 0 } else { 1 },
                })
            }
        }
    }

    /// Check with detailed result (used to populate RateLimit response headers).
    pub async fn check_detailed(&self, key: &RateLimitKey, rpm: u32) -> Result<RateLimitResult, PolicyError> {
        match self {
            Self::InMemory { buckets, default_algorithm } => {
                let map_key = key.to_string();
                let mut entry = buckets.entry(map_key).or_insert_with(|| {
                    match default_algorithm {
                        RateLimitAlgorithm::TokenBucket => BucketEntry::Token(TokenBucket::new(rpm)),
                        RateLimitAlgorithm::SlidingWindow => BucketEntry::Sliding(SlidingWindow::new(rpm, 60)),
                    }
                });

                let result = match entry.value_mut() {
                    BucketEntry::Token(b) => b.try_consume(),
                    BucketEntry::Sliding(w) => w.try_consume(),
                };

                Ok(result)
            }
            Self::Redis { client, shas } => {
                let redis_key = key.to_string();
                let result = Self::redis_check(client, shas, redis_key, rpm)
                    .await
                    .map_err(|e| PolicyError::Internal(e.to_string()))?;

                let allowed = result.first().copied().unwrap_or(0) == 1;
                let remaining = result.get(1).copied().unwrap_or(0) as u32;

                Ok(RateLimitResult {
                    allowed,
                    limit: rpm,
                    remaining,
                    reset_after_secs: 60,
                    retry_after_secs: if allowed { 0 } else { 1 },
                })
            }
        }
    }

    /// Periodic cleanup of idle in-memory buckets (no-op for Redis).
    pub fn cleanup(&self, idle_threshold: Duration) {
        if let Self::InMemory { buckets, .. } = self {
            let now = Instant::now();
            buckets.retain(|_, entry| {
                match entry {
                    BucketEntry::Token(b) => now.duration_since(b.last_refill) < idle_threshold,
                    BucketEntry::Sliding(w) => {
                        w.timestamps.back().is_some_and(|t| now.duration_since(*t) < idle_threshold)
                    }
                }
            });
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new_in_memory()
    }
}
