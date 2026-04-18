-- Create custom ENUM types using idempotent DO blocks
DO $$ BEGIN
    CREATE TYPE user_role AS ENUM ('super_admin', 'tenant_admin', 'user', 'read_only');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE user_status AS ENUM ('active', 'inactive', 'locked', 'pending');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE backend_provider_type AS ENUM (
        'open_ai', 'anthropic', 'google_vertex', 'aws_bedrock',
        'ollama', 'vllm', 'open_ai_compatible', 'rest', 'graphql', 'grpc', 'generic'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE health_status AS ENUM ('healthy', 'degraded', 'unhealthy', 'unknown');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE route_protocol AS ENUM ('rest', 'graphql', 'grpc', 'generic');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Tenants table
CREATE TABLE tenants (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL UNIQUE,
    plan            TEXT NOT NULL DEFAULT 'community',
    settings        JSONB NOT NULL DEFAULT '{}',
    license_key     TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    max_users       INTEGER NOT NULL DEFAULT 5,
    max_api_keys    INTEGER NOT NULL DEFAULT 10,
    max_backends    INTEGER NOT NULL DEFAULT 3,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tenants_slug ON tenants (slug);
