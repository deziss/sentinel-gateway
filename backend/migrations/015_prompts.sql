-- Prompt Management & Versioning
-- Versioned prompts with deployment labels (prod/staging/canary/dev).
-- Enables: A/B testing, canary rollout, prompt-as-config, deploy without redeploying code.

CREATE TABLE IF NOT EXISTS prompts (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name        VARCHAR(255) NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    content     TEXT NOT NULL,
    -- Declared input variables — used for template enforcement + validation.
    -- Format: {"var_name": {"type": "string", "required": true, "description": "..."}}
    variables   JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- Default model preferences (temperature, max_tokens, etc.) — applied if client omits.
    model_prefs JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- Optional default model/alias; client override wins.
    default_model VARCHAR(255),
    -- Free-form metadata (tags, description, author)
    metadata    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (tenant_id, name, version)
);

CREATE INDEX idx_prompts_tenant_name ON prompts(tenant_id, name);
CREATE INDEX idx_prompts_updated ON prompts(tenant_id, updated_at DESC);

-- Deployment labels (prod, staging, canary, dev, feature-flag names)
-- → the "active" version for a given label. Only one version per label at a time.
CREATE TABLE IF NOT EXISTS prompt_deployments (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    prompt_name VARCHAR(255) NOT NULL,
    label       VARCHAR(64)  NOT NULL,
    version     INTEGER NOT NULL,
    deployed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    deployed_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (tenant_id, prompt_name, label)
);

CREATE INDEX idx_prompt_deployments_lookup
    ON prompt_deployments(tenant_id, prompt_name, label);
