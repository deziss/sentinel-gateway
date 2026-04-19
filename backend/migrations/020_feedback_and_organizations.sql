-- ─────────────────────────────────────────────────────────────────────────────
-- 020_feedback_and_organizations.sql
--
-- Adds:
--   1. `llm_feedback`      — end-user thumbs-up/down + free-form comments keyed to llm_logs
--   2. `organizations`     — parent grouping for multiple tenants (Portkey-style "Org")
--   3. `tenant.organization_id` — FK from tenants to organizations (nullable; backwards compat)
-- ─────────────────────────────────────────────────────────────────────────────

-- ── Organizations (tenant-of-tenants) ──────────────────────────────────────
CREATE TABLE IF NOT EXISTS organizations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    plan        TEXT NOT NULL DEFAULT 'free',
    metadata    JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_organizations_slug ON organizations(slug);

ALTER TABLE tenants
    ADD COLUMN IF NOT EXISTS organization_id UUID
        REFERENCES organizations(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_tenants_organization_id ON tenants(organization_id);

-- ── LLM feedback ───────────────────────────────────────────────────────────
-- Keyed to llm_logs (request_id if present, else llm_log id).
CREATE TABLE IF NOT EXISTS llm_feedback (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    llm_log_id      UUID,
    request_id      TEXT,
    -- 1=thumbs-up, -1=thumbs-down, 0=neutral
    rating          SMALLINT NOT NULL CHECK (rating BETWEEN -1 AND 1),
    comment         TEXT,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_llm_feedback_tenant ON llm_feedback(tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_feedback_log ON llm_feedback(llm_log_id) WHERE llm_log_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_llm_feedback_request ON llm_feedback(request_id) WHERE request_id IS NOT NULL;
