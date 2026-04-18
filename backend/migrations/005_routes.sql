-- Routes table (proxy routing rules)
CREATE TABLE routes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    protocol        route_protocol NOT NULL,
    path_pattern    TEXT NOT NULL,
    backend_id      UUID NOT NULL REFERENCES backends(id) ON DELETE CASCADE,
    strip_prefix    BOOLEAN NOT NULL DEFAULT false,
    rewrite_rules   JSONB NOT NULL DEFAULT '{}',
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_routes_tenant_id ON routes (tenant_id);
CREATE INDEX idx_routes_path_pattern ON routes (tenant_id, path_pattern);
