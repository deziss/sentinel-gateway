-- Backends table (upstream proxy targets)
CREATE TABLE backends (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id               UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name                    TEXT NOT NULL,
    provider_type           backend_provider_type NOT NULL,
    endpoint                TEXT NOT NULL,
    encrypted_credentials   TEXT,
    health_status           health_status NOT NULL DEFAULT 'unknown',
    priority                INTEGER NOT NULL DEFAULT 100,
    weight                  INTEGER NOT NULL DEFAULT 1,
    timeout_ms              INTEGER NOT NULL DEFAULT 30000,
    max_retries             INTEGER NOT NULL DEFAULT 3,
    is_active               BOOLEAN NOT NULL DEFAULT true,
    last_health_check       TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_backends_tenant_id ON backends (tenant_id);
CREATE INDEX idx_backends_health ON backends (tenant_id, health_status, is_active);
