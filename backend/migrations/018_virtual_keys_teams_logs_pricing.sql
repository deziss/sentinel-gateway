-- =============================================================================
-- P0 Feature Parity: Virtual Keys + Teams + LLM Logs + Pricing Overrides
-- =============================================================================

-- ── Teams (multi-tenant group abstraction within a tenant) ────────────────
CREATE TABLE IF NOT EXISTS teams (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL,
    description TEXT,
    settings    JSONB NOT NULL DEFAULT '{}',
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, slug)
);

CREATE INDEX idx_teams_tenant ON teams (tenant_id) WHERE is_active = true;

-- Team membership (many-to-many: users ↔ teams)
CREATE TABLE IF NOT EXISTS team_members (
    team_id     UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL DEFAULT 'member'
        CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (team_id, user_id)
);

CREATE INDEX idx_team_members_user ON team_members (user_id);

-- ── Virtual Keys (consumer key → provider credential mapping) ─────────────
-- Portkey-style: one virtual key maps to provider credentials + budget/rate policies.
-- Consumers get the virtual key (safe to rotate/revoke) and never see real credentials.
CREATE TABLE IF NOT EXISTS virtual_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    team_id         UUID REFERENCES teams(id) ON DELETE SET NULL,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    key_hash        TEXT NOT NULL UNIQUE,
    key_prefix      TEXT NOT NULL,           -- first 12 chars for identification
    -- Backend this virtual key proxies to
    backend_id      UUID NOT NULL REFERENCES backends(id) ON DELETE CASCADE,
    -- Optional model allow-list (NULL = all models allowed on the backend)
    allowed_models  TEXT[],
    -- Per-key rate limit (requests per minute). NULL = inherit tenant default
    rate_limit_rpm  INTEGER,
    -- Per-key token limit (tokens per minute)
    token_limit_tpm INTEGER,
    -- Per-key daily budget (USD)
    budget_daily    DOUBLE PRECISION,
    -- Per-key monthly budget (USD)
    budget_monthly  DOUBLE PRECISION,
    metadata        JSONB NOT NULL DEFAULT '{}',
    is_active       BOOLEAN NOT NULL DEFAULT true,
    expires_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_vkeys_hash ON virtual_keys (key_hash) WHERE is_active = true;
CREATE INDEX idx_vkeys_tenant ON virtual_keys (tenant_id, created_at DESC);
CREATE INDEX idx_vkeys_team ON virtual_keys (team_id) WHERE team_id IS NOT NULL;

-- ── LLM Logs (full request/response capture for audit + replay) ───────────
-- Partitioned by month (same strategy as audit_logs + usage_records).
-- PII-redacted content stored in the `request` / `response` fields — the PII
-- detection pipeline runs BEFORE writing. Raw content is never persisted.
CREATE TABLE IF NOT EXISTS llm_logs (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL,
    user_id         UUID,
    api_key_id      UUID,
    virtual_key_id  UUID,
    backend_id      UUID,
    model           TEXT NOT NULL,
    provider        TEXT NOT NULL,
    endpoint_path   TEXT NOT NULL,           -- e.g. "/v1/chat/completions"
    -- Redacted request body (messages + params, NOT secrets)
    request         JSONB NOT NULL,
    -- Redacted response body (content + usage, NOT headers)
    response        JSONB,
    status_code     INTEGER NOT NULL,
    tokens_input    BIGINT NOT NULL DEFAULT 0,
    tokens_output   BIGINT NOT NULL DEFAULT 0,
    cost_usd        DOUBLE PRECISION NOT NULL DEFAULT 0,
    latency_ms      BIGINT NOT NULL DEFAULT 0,
    -- Derived flags for fast filtering
    cached          BOOLEAN NOT NULL DEFAULT false,
    pii_detected    BOOLEAN NOT NULL DEFAULT false,
    error           TEXT,
    -- trace_id for OTel correlation
    trace_id        TEXT,
    request_id      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (created_at, id)
) PARTITION BY RANGE (created_at);

CREATE TABLE llm_logs_default PARTITION OF llm_logs DEFAULT;

-- Composite indexes for the 5 common query shapes
CREATE INDEX idx_llm_logs_tenant ON llm_logs (tenant_id, created_at DESC);
CREATE INDEX idx_llm_logs_user ON llm_logs (tenant_id, user_id, created_at DESC)
    WHERE user_id IS NOT NULL;
CREATE INDEX idx_llm_logs_model ON llm_logs (tenant_id, model, created_at DESC);
CREATE INDEX idx_llm_logs_errors ON llm_logs (tenant_id, created_at DESC)
    WHERE status_code >= 400;
CREATE INDEX idx_llm_logs_vkey ON llm_logs (virtual_key_id, created_at DESC)
    WHERE virtual_key_id IS NOT NULL;

-- Pre-create 3 months of partitions
DO $$
DECLARE
    m INTEGER;
    d DATE;
BEGIN
    FOR m IN 0..3 LOOP
        d := DATE_TRUNC('month', NOW() + (m || ' months')::INTERVAL)::DATE;
        PERFORM create_monthly_partition('llm_logs', d);
    END LOOP;
END $$;

-- ── Pricing Overrides (per-tenant model pricing with margin markup) ───────
CREATE TABLE IF NOT EXISTS tenant_pricing (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    model            TEXT NOT NULL,
    -- Override input price (USD per 1M tokens). NULL = use default.
    input_per_1m     DOUBLE PRECISION,
    -- Override output price (USD per 1M tokens). NULL = use default.
    output_per_1m    DOUBLE PRECISION,
    -- Multiplicative markup (e.g., 1.3 = 30% margin). Applied AFTER overrides.
    markup_multiplier DOUBLE PRECISION NOT NULL DEFAULT 1.0
        CHECK (markup_multiplier >= 0),
    is_active        BOOLEAN NOT NULL DEFAULT true,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, model)
);

CREATE INDEX idx_pricing_tenant ON tenant_pricing (tenant_id) WHERE is_active = true;

-- Add optional team_id + virtual_key_id to usage_records for billing grouping
ALTER TABLE usage_records ADD COLUMN IF NOT EXISTS team_id UUID;
ALTER TABLE usage_records ADD COLUMN IF NOT EXISTS virtual_key_id UUID;
CREATE INDEX IF NOT EXISTS idx_usage_team ON usage_records (team_id, created_at DESC)
    WHERE team_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_usage_vkey ON usage_records (virtual_key_id, created_at DESC)
    WHERE virtual_key_id IS NOT NULL;
