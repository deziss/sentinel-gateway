-- Provider Data-Policy classification
-- Lets tenants exclude providers based on data-handling guarantees.
-- Matches OpenRouter / Portkey data-policy filters.
--
-- Policies (ordered most-strict → least-strict):
--   strict        — zero logging, zero retention, no training, on-premise only
--   no_training   — provider contractually won't train on submitted data
--   no_retention  — 0-day retention (e.g., OpenAI ZDR, Anthropic zero retention)
--   standard      — provider's default policy (may include logging for abuse detection)
--
-- Clients can send `X-Min-Data-Policy: no_training` to only use providers with
-- at least that level of guarantee.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'data_policy') THEN
        CREATE TYPE data_policy AS ENUM (
            'standard',
            'no_retention',
            'no_training',
            'strict'
        );
    END IF;
END
$$;

ALTER TABLE backends
    ADD COLUMN IF NOT EXISTS data_policy data_policy NOT NULL DEFAULT 'standard';

CREATE INDEX IF NOT EXISTS idx_backends_data_policy
    ON backends(tenant_id, data_policy)
    WHERE is_active = true;
