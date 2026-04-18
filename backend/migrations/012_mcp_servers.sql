-- MCP (Model Context Protocol) server registry
-- Stores configured upstream MCP servers and their connection state.

CREATE TABLE IF NOT EXISTS mcp_servers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name        VARCHAR(255) NOT NULL,
    url         TEXT NOT NULL,
    description TEXT,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    is_healthy  BOOLEAN NOT NULL DEFAULT false,
    -- Discovery metadata (cached from last connection)
    tools_count     INTEGER NOT NULL DEFAULT 0,
    resources_count INTEGER NOT NULL DEFAULT 0,
    prompts_count   INTEGER NOT NULL DEFAULT 0,
    -- Connection state
    last_connected_at   TIMESTAMPTZ,
    last_discovery_at   TIMESTAMPTZ,
    last_error          TEXT,
    -- Auth for the upstream MCP server (optional)
    auth_type       VARCHAR(50),  -- 'none', 'bearer', 'api_key', 'basic'
    auth_credential TEXT,         -- Encrypted credential (via FieldEncryptor)
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (tenant_id, name)
);

CREATE INDEX idx_mcp_servers_tenant ON mcp_servers(tenant_id);
CREATE INDEX idx_mcp_servers_active ON mcp_servers(tenant_id, is_active);
