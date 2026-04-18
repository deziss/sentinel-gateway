//! Test fixture builders.

use gateway_db::models::{
    api_key::CreateApiKey,
    tenant::CreateTenant,
    user::{CreateUser, UserRole},
};
use uuid::Uuid;

pub fn test_tenant(slug: &str) -> CreateTenant {
    CreateTenant {
        name: format!("Test Tenant {slug}"),
        slug: slug.to_string(),
        plan: "community".to_string(),
        max_users: 100,
        max_api_keys: 100,
        max_backends: 100,
    }
}

pub fn test_user(tenant_id: Uuid, email: &str, password_hash: &str, role: UserRole) -> CreateUser {
    CreateUser {
        tenant_id,
        email: email.to_string(),
        password_hash: password_hash.to_string(),
        role,
    }
}

pub fn test_api_key(tenant_id: Uuid, user_id: Uuid, key_hash: &str) -> CreateApiKey {
    CreateApiKey {
        tenant_id,
        user_id,
        key_hash: key_hash.to_string(),
        name: "test-key".to_string(),
        scopes: vec!["*".to_string()],
        rate_limit_rpm: Some(60),
        budget_daily: Some(10.0),
        budget_monthly: Some(100.0),
        expires_at: None,
    }
}
