use crate::error::{DbError, DbResult};
use crate::models::sso::{
    CreateSsoProvider, SsoAuthState, SsoIdentity, SsoProvider, UpsertSsoIdentity,
};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct SsoProviderRepository {
    pool: DbPool,
}

impl SsoProviderRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateSsoProvider) -> DbResult<SsoProvider> {
        let id = Uuid::new_v4();
        let provider = sqlx::query_as::<_, SsoProvider>(
            r#"
            INSERT INTO sso_providers
                (id, tenant_id, kind, display_name, slug, client_id, client_secret,
                 issuer_url, authorize_url, token_url, userinfo_url, jwks_url,
                 scopes, default_role, auto_provision, metadata)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,
                    COALESCE($13,'openid profile email'), $14,
                    COALESCE($15,true), COALESCE($16,'{}'::jsonb))
            RETURNING id, tenant_id, kind, display_name, slug, client_id, client_secret,
                      issuer_url, authorize_url, token_url, userinfo_url, jwks_url,
                      scopes, default_role, auto_provision, is_active, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(&input.kind)
        .bind(&input.display_name)
        .bind(&input.slug)
        .bind(&input.client_id)
        .bind(&input.client_secret)
        .bind(&input.issuer_url)
        .bind(&input.authorize_url)
        .bind(&input.token_url)
        .bind(&input.userinfo_url)
        .bind(&input.jwks_url)
        .bind(&input.scopes)
        .bind(&input.default_role)
        .bind(input.auto_provision)
        .bind(&input.metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(provider)
    }

    pub async fn find_by_slug(&self, tenant_id: Uuid, slug: &str) -> DbResult<SsoProvider> {
        sqlx::query_as::<_, SsoProvider>(
            r#"
            SELECT id, tenant_id, kind, display_name, slug, client_id, client_secret,
                   issuer_url, authorize_url, token_url, userinfo_url, jwks_url,
                   scopes, default_role, auto_provision, is_active, metadata,
                   created_at, updated_at
            FROM sso_providers WHERE tenant_id = $1 AND slug = $2 AND is_active = true
            "#,
        )
        .bind(tenant_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("SSO provider {slug}")))
    }

    pub async fn find_by_id(&self, id: Uuid) -> DbResult<SsoProvider> {
        sqlx::query_as::<_, SsoProvider>(
            r#"
            SELECT id, tenant_id, kind, display_name, slug, client_id, client_secret,
                   issuer_url, authorize_url, token_url, userinfo_url, jwks_url,
                   scopes, default_role, auto_provision, is_active, metadata,
                   created_at, updated_at
            FROM sso_providers WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("SSO provider {id}")))
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<SsoProvider>> {
        let rows = sqlx::query_as::<_, SsoProvider>(
            r#"
            SELECT id, tenant_id, kind, display_name, slug, client_id, client_secret,
                   issuer_url, authorize_url, token_url, userinfo_url, jwks_url,
                   scopes, default_role, auto_provision, is_active, metadata,
                   created_at, updated_at
            FROM sso_providers WHERE tenant_id = $1 ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE sso_providers SET is_active = false, updated_at = NOW() WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub struct SsoIdentityRepository {
    pool: DbPool,
}

impl SsoIdentityRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Upsert an SSO identity. Key is (provider_id, provider_user_id).
    pub async fn upsert(&self, input: UpsertSsoIdentity) -> DbResult<SsoIdentity> {
        let identity = sqlx::query_as::<_, SsoIdentity>(
            r#"
            INSERT INTO sso_identities
                (id, user_id, provider_id, provider_user_id, provider_email,
                 provider_username, raw_profile, access_token_enc, refresh_token_enc,
                 token_expires_at, last_login_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            ON CONFLICT (provider_id, provider_user_id) DO UPDATE SET
                provider_email = $4,
                provider_username = $5,
                raw_profile = $6,
                access_token_enc = $7,
                refresh_token_enc = $8,
                token_expires_at = $9,
                last_login_at = NOW()
            RETURNING id, user_id, provider_id, provider_user_id, provider_email,
                      provider_username, raw_profile, access_token_enc, refresh_token_enc,
                      token_expires_at, last_login_at, created_at
            "#,
        )
        .bind(input.user_id)
        .bind(input.provider_id)
        .bind(&input.provider_user_id)
        .bind(&input.provider_email)
        .bind(&input.provider_username)
        .bind(&input.raw_profile)
        .bind(&input.access_token_enc)
        .bind(&input.refresh_token_enc)
        .bind(input.token_expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(identity)
    }

    /// Find an identity by (provider_id, provider_user_id). Used on every SSO login.
    pub async fn find(&self, provider_id: Uuid, provider_user_id: &str) -> DbResult<Option<SsoIdentity>> {
        let row = sqlx::query_as::<_, SsoIdentity>(
            r#"
            SELECT id, user_id, provider_id, provider_user_id, provider_email,
                   provider_username, raw_profile, access_token_enc, refresh_token_enc,
                   token_expires_at, last_login_at, created_at
            FROM sso_identities WHERE provider_id = $1 AND provider_user_id = $2
            "#,
        )
        .bind(provider_id)
        .bind(provider_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_for_user(&self, user_id: Uuid) -> DbResult<Vec<SsoIdentity>> {
        let rows = sqlx::query_as::<_, SsoIdentity>(
            r#"
            SELECT id, user_id, provider_id, provider_user_id, provider_email,
                   provider_username, raw_profile, access_token_enc, refresh_token_enc,
                   token_expires_at, last_login_at, created_at
            FROM sso_identities WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn unlink(&self, id: Uuid, user_id: Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM sso_identities WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub struct SsoAuthStateRepository {
    pool: DbPool,
}

impl SsoAuthStateRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        state: &str,
        provider_id: Uuid,
        code_verifier: Option<&str>,
        nonce: Option<&str>,
        redirect_after: Option<&str>,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO sso_auth_states (state, provider_id, code_verifier, nonce, redirect_after)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(state)
        .bind(provider_id)
        .bind(code_verifier)
        .bind(nonce)
        .bind(redirect_after)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch and delete in one atomic step (state is single-use).
    pub async fn consume(&self, state: &str) -> DbResult<SsoAuthState> {
        let row = sqlx::query_as::<_, SsoAuthState>(
            r#"
            DELETE FROM sso_auth_states
            WHERE state = $1 AND expires_at > NOW()
            RETURNING state, provider_id, code_verifier, nonce, redirect_after, created_at, expires_at
            "#,
        )
        .bind(state)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound("sso auth state (expired or used)".to_string()))?;
        Ok(row)
    }

    /// Periodic cleanup of expired states.
    pub async fn cleanup_expired(&self) -> DbResult<u64> {
        let result = sqlx::query("DELETE FROM sso_auth_states WHERE expires_at <= NOW()")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
