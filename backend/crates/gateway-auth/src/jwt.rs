use crate::error::AuthError;
use crate::roles::Role;
use crate::token_blacklist::TokenBlacklist;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Tenant ID
    pub tid: String,
    /// Role
    pub role: Role,
    /// Token type: "access" or "refresh"
    pub typ: String,
    /// Issued at (unix timestamp)
    pub iat: i64,
    /// Expiry (unix timestamp)
    pub exp: i64,
    /// JWT ID (for revocation)
    pub jti: String,
}

impl Claims {
    pub fn user_id(&self) -> Uuid {
        Uuid::parse_str(&self.sub).expect("valid UUID in sub claim")
    }

    pub fn tenant_id(&self) -> Uuid {
        Uuid::parse_str(&self.tid).expect("valid UUID in tid claim")
    }
}

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_ttl_minutes: i64,
    refresh_ttl_days: i64,
}

impl JwtService {
    pub fn new(
        private_key_pem: &[u8],
        public_key_pem: &[u8],
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> Result<Self, AuthError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_key_pem)
            .map_err(|e| AuthError::Internal(format!("Invalid private key: {e}")))?;
        let decoding_key = DecodingKey::from_rsa_pem(public_key_pem)
            .map_err(|e| AuthError::Internal(format!("Invalid public key: {e}")))?;
        Ok(Self {
            encoding_key,
            decoding_key,
            access_ttl_minutes,
            refresh_ttl_days,
        })
    }

    pub fn issue_access_token(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        role: Role,
    ) -> Result<String, AuthError> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            tid: tenant_id.to_string(),
            role,
            typ: "access".to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::minutes(self.access_ttl_minutes)).timestamp(),
            jti: Uuid::new_v4().to_string(),
        };
        encode(&Header::new(Algorithm::RS256), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("Token encode error: {e}")))
    }

    pub fn issue_refresh_token(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        role: Role,
    ) -> Result<String, AuthError> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            tid: tenant_id.to_string(),
            role,
            typ: "refresh".to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::days(self.refresh_ttl_days)).timestamp(),
            jti: Uuid::new_v4().to_string(),
        };
        encode(&Header::new(Algorithm::RS256), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("Token encode error: {e}")))
    }

    pub fn validate(&self, token: &str) -> Result<Claims, AuthError> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::TokenInvalid(e.to_string()),
            })
    }

    /// Validate a token and check if it has been revoked.
    pub fn validate_with_blacklist(
        &self,
        token: &str,
        blacklist: &TokenBlacklist,
    ) -> Result<Claims, AuthError> {
        let claims = self.validate(token)?;
        if blacklist.is_revoked(&claims.jti) {
            return Err(AuthError::TokenRevoked);
        }
        Ok(claims)
    }
}
