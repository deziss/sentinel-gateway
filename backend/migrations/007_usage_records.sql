-- Usage records (token/cost tracking)
CREATE TABLE usage_records (
    id              UUID DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    api_key_id      UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    backend_id      UUID NOT NULL REFERENCES backends(id) ON DELETE CASCADE,
    model           TEXT,
    tokens_input    BIGINT NOT NULL DEFAULT 0,
    tokens_output   BIGINT NOT NULL DEFAULT 0,
    cost_usd        DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    latency_ms      BIGINT NOT NULL DEFAULT 0,
    status_code     INTEGER NOT NULL DEFAULT 200,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

CREATE TABLE usage_records_default PARTITION OF usage_records DEFAULT;

CREATE INDEX idx_usage_tenant ON usage_records (tenant_id, created_at DESC);
CREATE INDEX idx_usage_model  ON usage_records (tenant_id, model, created_at DESC);
CREATE INDEX idx_usage_apikey ON usage_records (api_key_id, created_at DESC);
