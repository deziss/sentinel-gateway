-- Stores license entitlements and validation state per tenant.
-- `updated_at` is maintained by application code (TenantLicenseRepository::update)
-- to match the convention of migrations 001–020; no trigger is added here.

CREATE TABLE IF NOT EXISTS tenant_licenses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- The license key itself (masked or encrypted in UI, but stored for heartbeats)
    license_key TEXT NOT NULL,

    -- online (heartbeat required) or offline (Ed25519 signed token)
    license_type TEXT NOT NULL DEFAULT 'online',

    -- active, expired, revoked, suspended
    status TEXT NOT NULL DEFAULT 'active',

    -- Plan snapshot (community, professional, enterprise)
    plan TEXT NOT NULL DEFAULT 'community',

    -- JSONB blob of FeatureFlags for 1µs resolution without plan lookups
    entitlements JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Hardware fingerprint (activation lock)
    fingerprint TEXT,

    expires_at TIMESTAMPTZ,
    last_validated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    last_reported_at TIMESTAMPTZ,

    -- For offline licenses: stores the raw JWT/token
    offline_token TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- One active license per tenant. The UNIQUE constraint also provides the
    -- tenant_id lookup index implicitly — no separate idx_tenant_licenses_tenant_id needed.
    CONSTRAINT unique_active_tenant_license UNIQUE (tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_tenant_licenses_status ON tenant_licenses(status);
-- Composite index covers the hot path used by list_for_heartbeat / list_for_usage_report.
CREATE INDEX IF NOT EXISTS idx_tenant_licenses_active_online
    ON tenant_licenses(status, license_type)
    WHERE status = 'active' AND license_type = 'online';
