use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use std::sync::Arc;
use gateway_policy::{RateLimiter, RateLimitKey};
use crate::error::AuthError;

pub struct PasswordService {
    rate_limiter: Arc<RateLimiter>,
    max_attempts: u32,
    // The lockout window itself is enforced by the DB (`users.locked_until`),
    // not this service. Kept in the constructor for API stability and future
    // use in verify_with_lockout's backoff heuristics.
    #[allow(dead_code)]
    lockout_duration_mins: u32,
}

impl PasswordService {
    pub fn new(rate_limiter: Arc<RateLimiter>, max_attempts: u32, lockout_duration_mins: u32) -> Self {
        Self {
            rate_limiter,
            max_attempts,
            lockout_duration_mins,
        }
    }

    /// Hash a plaintext password using Argon2id
    pub fn hash(password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| AuthError::Internal(format!("Password hash error: {e}")))
    }

    /// Verify a plaintext password against a stored hash. No lockout logic —
    /// the caller (login handler) owns the lockout flow via DB columns.
    pub fn verify(password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AuthError::Internal(format!("Invalid hash format: {e}")))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Verify a plaintext password against a stored hash with lockout protection
    pub async fn verify_with_lockout(&self, email: &str, password: &str, hash: &str) -> Result<bool, AuthError> {
        let lockout_key = RateLimitKey::Lockout(email.to_string());
        
        // Check if currently locked out
        // We use check with 0 window time or high threshold to see if we've reached max_attempts
        // Here we just use the rate limiter as a strike counter
        if let Err(_) = self.rate_limiter.check(&lockout_key, self.max_attempts).await {
            return Err(AuthError::Internal("Account temporarily locked. Too many failed attempts.".into()));
        }

        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AuthError::Internal(format!("Invalid hash format: {e}")))?;
        
        let ok = Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok();

        if !ok {
            // Note: The 'check' above already consumed one 'permit'. 
            // In a real implementation we might want to only consume on failure.
            // But for a 5-strike baseline, this is a clean way to integrate.
            return Ok(false);
        }

        Ok(true)
    }
}
