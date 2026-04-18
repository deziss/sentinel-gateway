use chrono::Utc;
use serde::{Deserialize, Serialize};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use crate::error::LicenseError;
use crate::features::{FeatureFlags, Plan};

/// Claims embedded in a license JWT
#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseClaims {
    pub sub: String,   // tenant_id
    pub plan: String,
    pub exp: i64,
    pub iat: i64,
    pub features: serde_json::Value,
}

pub struct LicenseValidator {
    decoding_key: DecodingKey,
    grace_period_days: i64,
}

impl LicenseValidator {
    pub fn new(public_key_pem: &[u8], grace_period_days: i64) -> Result<Self, LicenseError> {
        let decoding_key = DecodingKey::from_rsa_pem(public_key_pem)
            .map_err(|e| LicenseError::Internal(format!("Invalid license key: {e}")))?;
        Ok(Self {
            decoding_key,
            grace_period_days,
        })
    }

    /// Validate a license key string and return feature flags.
    pub fn validate(&self, license_key: &str) -> Result<FeatureFlags, LicenseError> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = false; // we check expiry ourselves with grace period

        let claims = decode::<LicenseClaims>(license_key, &self.decoding_key, &validation)
            .map_err(|_| LicenseError::Invalid)?
            .claims;

        // Check expiry with grace period
        let now = Utc::now().timestamp();
        let grace = self.grace_period_days * 86400;
        if claims.exp > 0 && now > claims.exp + grace {
            return Err(LicenseError::Expired);
        }

        let plan = Plan::from_str(&claims.plan);
        let mut features = FeatureFlags::for_plan(plan);
        
        // Custom feature overrides from claims
        if let Some(obj) = claims.features.as_object() {
            if let Some(v) = obj.get("max_backends").and_then(|v| v.as_u64()) {
                features.max_backends = v as u32;
            }
            if let Some(v) = obj.get("sso_enabled").and_then(|v| v.as_bool()) {
                features.sso_enabled = v;
            }
            // Add other feature gates here...
        }

        Ok(features)
    }

    /// Generate a community (no-license) feature set for unlicensed deployments.
    pub fn community_features() -> FeatureFlags {
        FeatureFlags::for_plan(Plan::Community)
    }
}
