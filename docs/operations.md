# Sentinel Gateway — Operations Runbook

## SLOs

| SLI | Target | Measurement |
|-----|--------|-------------|
| Availability | 99.9% (30 days) | `sli:http_request_success_ratio:5m` |
| Latency p95 | < 500 ms | `http_request_duration_seconds` histogram |
| Latency p99 | < 2 s | same |
| Backend health | ≥1 healthy per tenant | `backend_health_status` gauge |
| Error budget burn rate | < 1.0 (1h window) | `sli:error_budget_burn_rate:1h` |

Budget at 99.9% over 30d = **43 min 12 sec** of downtime per month.

## Alert Severity

| Alert | Severity | Action window |
|-------|----------|---------------|
| CriticalErrorRate (>5%) | `critical` | Immediate — page oncall |
| HighLatencyP99 (>2s) | `critical` | Immediate |
| AllBackendsUnhealthyForTenant | `critical` | Immediate — tenant is down |
| HighErrorRate (>0.1%) | `warning` | 30 min response |
| HighLatencyP95 (>500ms) | `warning` | 30 min response |
| BackendUnhealthy | `warning` | Investigate within 1h |
| HighRateLimitRejectionRate | `warning` | Capacity review |
| BudgetExceededSpike | `warning` | Cost analysis |
| UnusualTokenUsage | `warning` | Abuse detection |
| HighActiveConnections | `warning` | Scale hint |

## Metrics

All metrics exposed at `GET /metrics` in Prometheus text format.

### HTTP (gateway-level)
| Metric | Type | Labels |
|--------|------|--------|
| `http_requests_total` | Counter | method, path, status |
| `http_request_duration_seconds` | Histogram | method, path |

### Proxy (upstream forwarding)
| Metric | Type | Labels |
|--------|------|--------|
| `proxy_requests_total` | Counter | tenant_id, backend, status |
| `proxy_request_duration_seconds` | Histogram | backend, model |

### LLM
| Metric | Type | Labels |
|--------|------|--------|
| `tokens_total` | Counter | tenant_id, model, direction (input/output) |
| `cost_usd_total` | Counter | tenant_id, model |

### Backend health + connections
| Metric | Type | Labels |
|--------|------|--------|
| `backend_health_status` | Gauge | tenant_id, backend_id (1=healthy, 0=unhealthy) |
| `active_connections` | Gauge | — |

### Policy enforcement
| Metric | Type | Labels |
|--------|------|--------|
| `rate_limited_total` | Counter | tenant_id, key_type |
| `budget_exceeded_total` | Counter | tenant_id |
| `errors_total` | Counter | kind, tenant_id |

## Runbooks

### Error rate above 5% (CriticalErrorRate)

1. Check `/readyz` — which dependency is down?
   ```bash
   curl -s https://gateway/readyz | jq '.checks'
   ```
2. If DB is slow, check connection pool:
   ```sql
   SELECT COUNT(*), state FROM pg_stat_activity
   WHERE datname = 'sentinel_gateway' GROUP BY state;
   ```
   Look for long-running queries or saturated pool. Restart replicas if pool exhausted with stuck queries (audit first).
3. Check recent deploys in Git — rollback if correlation.
4. Grafana → "Requests by Status Code" panel — what's the dominant error class?

### p99 latency above 2s

1. Grafana → "Proxy Request Latency by Backend" — is the spike isolated to one backend?
2. If a specific backend: check its circuit breaker state via `/api/v1/backends/:id` — should show `health_status` and `active_connections`.
3. If all backends: probable DB slowness.
   ```bash
   # Check pg_stat_statements (requires enable_query_stats=true)
   curl -s https://gateway/api/v1/admin/slow-queries?limit=10 \
       -H "Authorization: Bearer $SUPERADMIN_TOKEN"
   ```
4. Check `pg_stat_activity` for locks:
   ```sql
   SELECT pid, state, wait_event_type, wait_event, query
   FROM pg_stat_activity
   WHERE state != 'idle' AND now() - query_start > interval '1 second';
   ```

### Backend unhealthy

`BackendUnhealthy` fires when `HealthChecker` probe to `GET {endpoint}/health` fails repeatedly. Passive tracking also flips the status after 5 consecutive 5xx responses.

1. Is the backend URL reachable from the gateway network? (NetworkPolicy?)
2. Does the backend actually expose `/health`? Some upstreams don't — configure a different path in the Backend record's settings.
3. Check `last_health_check` timestamp — is the health worker running?
   ```bash
   kubectl logs -l app=sentinel-gateway | grep "health checker"
   ```

### All backends unhealthy (tenant down)

Skill rule: circuit breaker should degrade gracefully. If all backends are open:
- Gateway returns `CoreError::NoBackend` → client sees 503
- Investigate upstream outage first
- Check tenant's configured backends:
  ```bash
  curl -s https://gateway/api/v1/backends \
      -H "X-Tenant-ID: $TENANT" \
      -H "Authorization: Bearer $ADMIN" | jq .
  ```
- Verify health probe config per backend

### Rate limit rejections spiking

`HighRateLimitRejectionRate` fires when >100 req/s are rejected across all tenants. Could be:
- Legit traffic growth → raise tenant's `rate_limit_rpm` on their API keys
- Abuse → check `rate_limited_total{key_type="login_ip"}` for login brute force
- Misconfigured client → find the offender via audit logs:
  ```
  GET /api/v1/audit-logs?action=RateLimitExceeded&limit=100
  ```

### Budget exceeded spike

Indicates tenants hitting LLM cost hard limits:
```bash
# Who's affected?
curl -s https://gateway/metrics | grep "budget_exceeded_total"

# Their 30-day usage
curl -s https://gateway/api/v1/usage \
    -H "Authorization: Bearer $TENANT_TOKEN"
```

Options:
- Legitimate growth → raise budget on tenant record
- Cost anomaly → investigate `UnusualTokenUsage` correlation
- Bill accordingly — the gateway has already protected you

### Connection pool exhausted

Symptoms: slow DB queries, `error: pool timed out`, `HighErrorRate` firing.

1. Check pool saturation:
   ```sql
   SELECT COUNT(*) FROM pg_stat_activity WHERE datname = 'sentinel_gateway';
   ```
2. If close to `max_connections * replicas`, add PgBouncer in front:
   ```
   Gateway pods → PgBouncer (transaction mode) → PostgreSQL
   ```
3. Reduce `max_connections` per replica to `20`, let PgBouncer multiplex. Each PostgreSQL connection uses ~10 MB RAM.

### Webhook deliveries failing

Events land in `webhook_failures` table with exponential backoff retries.

```bash
# List recent DLQ entries
curl -s https://gateway/api/v1/webhooks/failures \
    -H "Authorization: Bearer $ADMIN" | jq .

# Force retry
curl -X POST https://gateway/api/v1/webhooks/failures/<id>/retry \
    -H "Authorization: Bearer $ADMIN"
```

Retries cap at 10 attempts with 60s * 2^attempt backoff (1min, 2min, 4min, ..., max 24h). After 10 failures → status=`abandoned`.

### License expired / revoked

`GET /api/v1/license/status` shows the current state:
- `grace_period` → still serving, but log banner warns users. Renew before `grace_until`.
- `invalid` → license rejected. Gateway degrades to Community features immediately.

Force re-activation after renewal:
```bash
curl -X POST https://gateway/api/v1/license/activate \
    -H "Authorization: Bearer $SUPERADMIN_TOKEN"
```

### Cache stampede after deploy

Symptoms: big latency spike on first ~1 minute after rollout; all caches cold.

Mitigations already in place:
- `ApiKeyCache` is per-replica (no shared stampede)
- `SemanticCache` is per-replica + opt-in per tenant
- DB has partial indexes covering hot queries

If you see this on first deploy ever: enable semantic cache warmup via scripts/bench or synthetic traffic before cutover.

### Missing pg_stat_statements data

`/admin/slow-queries` returns 501:
1. Confirm `database.enable_query_stats=true`
2. Confirm extension installed:
   ```sql
   SELECT * FROM pg_extension WHERE extname='pg_stat_statements';
   ```
3. Extension requires superuser. Managed DBs typically preinstall it (AWS RDS: enable in parameter group). If DIY:
   ```sql
   -- In postgresql.conf:
   shared_preload_libraries = 'pg_stat_statements'
   -- Restart PostgreSQL, then:
   CREATE EXTENSION pg_stat_statements;
   ```

## Backup + Restore

Skill rule: continuous WAL archiving + daily base backups + **weekly test restore** against staging.

```bash
# Daily base backup (example with pg_basebackup)
pg_basebackup -D /backups/$(date +%F) -U replicator -Fp -Xs -P

# Continuous WAL archiving (postgresql.conf)
archive_mode = on
archive_command = 'aws s3 cp %p s3://sentinel-wal/%f'
```

**Test the restore weekly** — untested backups are not backups.

## Upgrade Procedure

1. **Read the changelog** for breaking migrations.
2. **Run migrations against staging first**:
   ```bash
   gateway-server migrate
   ```
3. If any migration is >30s on production-size data, convert to a background job (skill rule). Safer alternatives:
   - `CREATE INDEX CONCURRENTLY` instead of plain `CREATE INDEX`
   - `ALTER TABLE ... ADD COLUMN ... DEFAULT NULL` then backfill, then `SET NOT NULL`
   - Online schema change tools (pt-online-schema-change, gh-ost) for `>1M` row tables
4. **Roll pods one at a time** with `rollingUpdate.maxUnavailable=1`.
5. **Monitor** the Grafana "Error Budget Burn Rate" panel during rollout.
6. **Rollback plan** — keep previous image tag in registry for 30 days.

## Capacity Planning

| Resource | Threshold | Action |
|----------|-----------|--------|
| Gateway CPU > 60% sustained | Add replicas | HPA handles this |
| PostgreSQL reads > 8k QPS | Add read replica | Route audit + usage queries to replica |
| PostgreSQL writes > 4k QPS | Shard by `tenant_id` | Plan 6 months ahead |
| PostgreSQL storage > 2TB | Drop old partitions | `SELECT drop_old_partitions('audit_logs', 90);` |
| Redis memory > 70% | Scale Redis up/out | Cluster if >100 GB |
| OTEL Collector dropping spans | Increase sampling or scale collector | Check `otel_collector_exporter_send_failed_spans` |

## Security Incident Response

### Suspected credential leak

1. Revoke all API keys for the affected tenant:
   ```sql
   UPDATE api_keys SET is_active = false WHERE tenant_id = '<uuid>';
   ```
2. Force-logout all active sessions — rotate the JWT signing key:
   ```bash
   gateway-server generate-keys --output-dir keys-new
   # Atomically swap and restart
   ```
3. Audit log review:
   ```
   GET /api/v1/audit-logs?tenant_id=<uuid>&limit=200
   ```

### DDoS

- `HighRateLimitRejectionRate` alert catches it early
- At scale, add a CDN (Cloudflare, Fastly) in front — gateway's IP filter is per-tenant, not for edge blocking
- Per-IP login rate limit already enforces 10/min globally

### Data exfiltration via LLM

- PII detection (per-tenant config) intercepts outbound → `redact` or `block` modes
- Semantic cache prevents re-querying sensitive prompts
- Audit logs retain every LLM request (30/90/365 days by plan)
