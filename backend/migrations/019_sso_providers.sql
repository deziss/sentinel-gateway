-- =============================================================================
-- SSO / OAuth2 / OIDC Providers + Identity linking
-- =============================================================================
--
-- Two tables:
--   - `sso_providers`: per-tenant configured OAuth2 / OIDC providers
--                      (Keycloak, Okta, Google, GitHub, Microsoft, generic)
--   - `sso_identities`: links a platform user to a provider identity.
--                       Each (provider, provider_user_id) is unique — multiple
--                       providers can link to the same user (account merging).

CREATE TABLE IF NOT EXISTS sso_providers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    -- Provider name: keycloak | okta | google | github | microsoft | oidc_generic
    kind            TEXT NOT NULL
        CHECK (kind IN ('keycloak','okta','google','github','microsoft','oidc_generic')),
    -- Display name shown on the login UI (e.g., "Sign in with Corp SSO").
    display_name    TEXT NOT NULL,
    -- Unique slug for URL routing: /auth/sso/:slug/authorize
    slug            TEXT NOT NULL,
    client_id       TEXT NOT NULL,
    -- Client secret — encrypted at rest via FieldEncryptor when encryption_key is set.
    client_secret   TEXT NOT NULL,
    -- OIDC discovery URL (issuer). Keycloak/Okta/Microsoft/generic.
    -- Leave empty for providers with fixed endpoints (Google, GitHub).
    issuer_url      TEXT,
    -- Explicit endpoints (override or fill in for non-OIDC like GitHub).
    authorize_url   TEXT,
    token_url       TEXT,
    userinfo_url    TEXT,
    jwks_url        TEXT,
    -- Scopes requested (space-separated). Defaults per provider applied in code.
    scopes          TEXT NOT NULL DEFAULT 'openid profile email',
    -- Default role for auto-provisioned users. NULL = use tenant default.
    default_role    TEXT DEFAULT 'user',
    -- Auto-create users on first login (JIT provisioning).
    auto_provision  BOOLEAN NOT NULL DEFAULT true,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    -- Free-form metadata (e.g., role mapping, claim extraction config).
    metadata        JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, slug)
);

CREATE INDEX idx_sso_providers_tenant ON sso_providers (tenant_id) WHERE is_active = true;

-- Links platform users to provider identities.
-- `provider_user_id` is the provider's canonical subject (e.g. Keycloak `sub`,
-- GitHub numeric ID, Google `sub`).
CREATE TABLE IF NOT EXISTS sso_identities (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id         UUID NOT NULL REFERENCES sso_providers(id) ON DELETE CASCADE,
    provider_user_id    TEXT NOT NULL,
    provider_email      TEXT,
    provider_username   TEXT,
    -- Raw profile returned by the provider (for debugging + claim inspection).
    raw_profile         JSONB,
    -- Last encrypted access token (optional — for fetching more profile info).
    access_token_enc    TEXT,
    -- Last encrypted refresh token.
    refresh_token_enc   TEXT,
    token_expires_at    TIMESTAMPTZ,
    last_login_at       TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider_id, provider_user_id)
);

CREATE INDEX idx_sso_identities_user ON sso_identities (user_id);
CREATE INDEX idx_sso_identities_email ON sso_identities (provider_email) WHERE provider_email IS NOT NULL;

-- Temporary state for OAuth2 authorization code flow.
-- Holds the (state, code_verifier) pair during the round-trip to the provider.
CREATE TABLE IF NOT EXISTS sso_auth_states (
    state           TEXT PRIMARY KEY,
    provider_id     UUID NOT NULL REFERENCES sso_providers(id) ON DELETE CASCADE,
    -- PKCE code verifier (S256 challenge). NULL for providers that don't use PKCE.
    code_verifier   TEXT,
    -- OIDC nonce for ID token validation.
    nonce           TEXT,
    redirect_after  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes')
);

CREATE INDEX idx_sso_auth_states_expires ON sso_auth_states (expires_at);
