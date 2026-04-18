use dashmap::DashMap;
use std::time::{Duration, Instant};

/// In-memory token revocation store.
///
/// Stores JTI -> expiry so revoked tokens are rejected until they would have
/// expired naturally, at which point they are garbage-collected.
pub struct TokenBlacklist {
    revoked: DashMap<String, Instant>,
}

impl TokenBlacklist {
    pub fn new() -> Self {
        Self {
            revoked: DashMap::new(),
        }
    }

    /// Mark a token as revoked. `ttl` is the remaining validity of the token.
    pub fn revoke(&self, jti: &str, ttl: Duration) {
        let expires_at = Instant::now() + ttl;
        self.revoked.insert(jti.to_string(), expires_at);
    }

    /// Check whether a JTI has been revoked and is still within its TTL.
    pub fn is_revoked(&self, jti: &str) -> bool {
        match self.revoked.get(jti) {
            Some(entry) => {
                if Instant::now() < *entry {
                    true
                } else {
                    // Expired entry — remove lazily
                    drop(entry);
                    self.revoked.remove(jti);
                    false
                }
            }
            None => false,
        }
    }

    /// Remove all expired entries. Call periodically from a background task.
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.revoked.retain(|_, expires_at| now < *expires_at);
    }
}
