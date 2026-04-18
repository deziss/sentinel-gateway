-- Guardrail Rules — per-tenant configuration for the guardrail pipeline.
--
-- Each row is one guardrail rule applied to LLM requests/responses.
-- The `kind` column determines which built-in or external guardrail to use,
-- and `config` holds its parameters (regex patterns, schemas, thresholds).

CREATE TABLE IF NOT EXISTS guardrail_rules (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    -- Friendly identifier, e.g. "block-ssn", "enforce-json-output"
    name        VARCHAR(255) NOT NULL,
    -- Built-in kind: 'regex' | 'length' | 'json_schema' | 'pii'
    -- External kinds (future): 'azure_content_safety' | 'bedrock_guardrails' | 'presidio'
    kind        VARCHAR(64) NOT NULL,
    -- 'pre_call' (before LLM) | 'post_call' (after LLM) | 'logging_only'
    stage       VARCHAR(32) NOT NULL,
    -- 'block' (reject) | 'redact' (modify content) | 'flag' (log only)
    mode        VARCHAR(32) NOT NULL,
    -- Tag for grouping (e.g., 'pii', 'jailbreak', 'schema_validation')
    category    VARCHAR(64) NOT NULL DEFAULT 'general',
    -- JSON config specific to the kind:
    -- regex:       { "patterns": ["\\d{3}-\\d{2}-\\d{4}"] }
    -- length:      { "max_chars": 10000 }
    -- json_schema: { "schema": { "type": "object", "required": ["answer"] } }
    -- pii:         { "types": ["email", "phone", "ssn"] }
    config      JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- Priority — lower = runs first. Useful for redact-then-validate chains.
    priority    INTEGER NOT NULL DEFAULT 100,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (tenant_id, name)
);

CREATE INDEX idx_guardrail_rules_active
    ON guardrail_rules(tenant_id, stage, priority)
    WHERE is_active = true;
