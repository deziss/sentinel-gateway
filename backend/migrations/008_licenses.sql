-- Licenses table
CREATE TABLE licenses (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id               UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    license_key             TEXT NOT NULL UNIQUE,
    plan                    TEXT NOT NULL DEFAULT 'community',
    features                JSONB NOT NULL DEFAULT '{}',
    activated_at            TIMESTAMPTZ,
    expires_at              TIMESTAMPTZ,
    hardware_fingerprint    TEXT,
    is_active               BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_licenses_tenant_id ON licenses (tenant_id);
CREATE INDEX idx_licenses_key ON licenses (license_key);
