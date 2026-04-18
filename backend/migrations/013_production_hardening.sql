-- =============================================================================
-- Production Hardening: Partial Indexes, Auto-Partitioning, Webhook DLQ
-- =============================================================================

-- ── Partial indexes (skip rows queries never touch) ────────────────────────

-- API key lookup (hot path — every authenticated request)
CREATE INDEX IF NOT EXISTS idx_api_keys_active_hash
    ON api_keys (key_hash)
    WHERE is_active = true;

-- User auth lookup by email within tenant
CREATE INDEX IF NOT EXISTS idx_users_active_email
    ON users (tenant_id, email)
    WHERE status = 'active';

-- Healthy backends for load balancer (read every route match)
CREATE INDEX IF NOT EXISTS idx_backends_healthy_active
    ON backends (tenant_id, priority, weight)
    WHERE is_active = true AND health_status IN ('healthy', 'unknown');

-- Active routes (hot path — every proxy request)
CREATE INDEX IF NOT EXISTS idx_routes_active_path
    ON routes (tenant_id, path_pattern)
    WHERE is_active = true;

-- Active webhook endpoints per tenant (fire on every audit event)
CREATE INDEX IF NOT EXISTS idx_webhooks_active_per_tenant
    ON webhook_endpoints (tenant_id, created_at)
    WHERE is_active = true;

-- Active licenses per tenant
CREATE INDEX IF NOT EXISTS idx_licenses_active
    ON licenses (tenant_id, expires_at)
    WHERE is_active = true;

-- ── Cursor pagination indexes ──────────────────────────────────────────────

-- Audit logs: already has (tenant_id, created_at DESC), add composite for cursor
CREATE INDEX IF NOT EXISTS idx_audit_logs_cursor
    ON audit_logs (tenant_id, created_at DESC, id);

-- Usage records cursor pagination
CREATE INDEX IF NOT EXISTS idx_usage_cursor
    ON usage_records (tenant_id, created_at DESC, id);

-- ── Webhook DLQ (Dead Letter Queue) ────────────────────────────────────────

CREATE TABLE IF NOT EXISTS webhook_failures (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    endpoint_id     UUID NOT NULL REFERENCES webhook_endpoints(id) ON DELETE CASCADE,
    event_type      TEXT NOT NULL,
    payload         JSONB NOT NULL,
    signature       TEXT NOT NULL,
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    last_attempt_at TIMESTAMPTZ,
    next_retry_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status          TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'retrying', 'failed', 'delivered', 'abandoned')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhook_failures_retry
    ON webhook_failures (next_retry_at)
    WHERE status IN ('pending', 'retrying');

CREATE INDEX idx_webhook_failures_tenant
    ON webhook_failures (tenant_id, created_at DESC);

-- ── Monthly partition auto-creation function ───────────────────────────────
-- Creates next 3 months of partitions ahead of time. Call from cron or
-- a background task. No extension needed (pure SQL).

CREATE OR REPLACE FUNCTION create_monthly_partition(
    parent_table TEXT,
    start_date DATE
) RETURNS VOID AS $$
DECLARE
    partition_name TEXT;
    end_date DATE;
BEGIN
    partition_name := parent_table || '_y' || TO_CHAR(start_date, 'YYYY') || 'm' || TO_CHAR(start_date, 'MM');
    end_date := (start_date + INTERVAL '1 month')::DATE;

    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I PARTITION OF %I FOR VALUES FROM (%L) TO (%L)',
        partition_name, parent_table, start_date, end_date
    );
END;
$$ LANGUAGE plpgsql;

-- Create partitions for the next 3 months (including current) for both tables
DO $$
DECLARE
    month_offset INTEGER;
    target_date DATE;
BEGIN
    FOR month_offset IN 0..3 LOOP
        target_date := DATE_TRUNC('month', NOW() + (month_offset || ' months')::INTERVAL)::DATE;
        PERFORM create_monthly_partition('audit_logs', target_date);
        PERFORM create_monthly_partition('usage_records', target_date);
    END LOOP;
END $$;

-- Helper: drop partitions older than retention period
CREATE OR REPLACE FUNCTION drop_old_partitions(
    parent_table TEXT,
    retention_days INTEGER
) RETURNS TABLE(dropped_partition TEXT) AS $$
DECLARE
    part RECORD;
    cutoff DATE;
BEGIN
    cutoff := (NOW() - (retention_days || ' days')::INTERVAL)::DATE;
    FOR part IN
        SELECT c.relname AS partition_name,
               PG_GET_EXPR(c.relpartbound, c.oid) AS bounds
        FROM pg_class c
        JOIN pg_inherits i ON i.inhrelid = c.oid
        JOIN pg_class p ON p.oid = i.inhparent
        WHERE p.relname = parent_table
          AND c.relname LIKE parent_table || '_y%'
    LOOP
        -- Very conservative: only drop if name indicates date < cutoff
        IF part.partition_name ~ ('^' || parent_table || '_y(\d{4})m(\d{2})$') THEN
            IF TO_DATE(SUBSTRING(part.partition_name FROM LENGTH(parent_table) + 3 FOR 7), 'YYYY"m"MM') < cutoff THEN
                EXECUTE format('DROP TABLE IF EXISTS %I', part.partition_name);
                dropped_partition := part.partition_name;
                RETURN NEXT;
            END IF;
        END IF;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- NOTE: pg_stat_statements is NOT created here. It's togglable via config
-- (`database.enable_query_stats=true` → CREATE EXTENSION at startup).
-- Requires superuser or managed DB with extension preinstalled.
