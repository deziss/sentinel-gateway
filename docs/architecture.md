# Sentinel Gateway — Architecture

## System Overview

Sentinel Gateway is a modular monolith written in Rust (Axum) that acts as a universal reverse proxy with specialized LLM routing, enterprise governance, and multi-tenant isolation. The repository is a Cargo workspace of **12 domain crates** that compose into a single `gateway-server` binary.

```
┌────────────────────────────────────────────────────────────────────┐
│                         Client Request                              │
└──────────────────────────────────┬─────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────┐
│  gateway-server (Axum 0.7)                                          │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────┐     │
│  │  Middleware stack (outermost first)                       │     │
│  │   1. TLS enforcement  (require_tls ? 426 : pass)          │     │
│  │   2. Telemetry         (tracing span + metrics + HSTS)    │     │
│  │   3. Tenant resolve    (header→JWT→subdomain→default)     │     │
│  │   4. Auth              (JWT or API key → AuthContext)     │     │
│  │   5. Role gate         (SuperAdmin/TenantAdmin/...)       │     │
│  │   6. Policy            (IP filter, rate limit, budget)    │     │
│  └───────────────────────────────────────────────────────────┘     │
│                                                                     │
│   Routes:                                                           │
│     /api/v1/auth/*       /api/v1/users/*    /api/v1/backends/*      │
│     /api/v1/tenants/*    /api/v1/webhooks/* /api/v1/v1/chat/*       │
│     /api/v1/guardrails/* /api/v1/prompts/*  /api/v1/mcp/*           │
│     /api/v1/admin/*      /healthz, /readyz, /metrics                │
│                                                                     │
│   Fallback proxy handler:  GatewayEngine (LB + CB + proxy)          │
└──────────────────────────────────┬─────────────────────────────────┘
                                   │
          ┌────────────────────────┼────────────────────────┐
          ▼                        ▼                        ▼
    PostgreSQL 16            Redis (optional)         Upstream backends
    (primary store)          (rate limit +            OpenAI, Anthropic,
                              distributed cache)      Google, Ollama, ...
```

## Crate Layout

| Crate | Role | Depends on |
|-------|------|-----------|
| `gateway-db` | SQLx pool, models, repositories | — |
| `gateway-auth` | JWT, password, API keys, RBAC, middleware | `gateway-db`, `gateway-policy` |
| `gateway-tenant` | Tenant resolution, sync service | `gateway-db`, `gateway-auth` |
| `gateway-license` | Offline + online activation, feature flags | `gateway-db` |
| `gateway-policy` | Rate limit (TokenBucket/SlidingWindow, in-memory + Redis EVALSHA), budget enforcer (in-memory + Redis INCRBYFLOAT), IP filter, guardrails, CEL cost, semantic | `gateway-db` |
| `gateway-core` | Proxy engine, LB, circuit breaker, WebSocket, GraphQL, gRPC | `gateway-db`, `gateway-auth`, `gateway-tenant` |
| `gateway-llm` | Providers, adapters, router, cost, cache, PII, smart routing | `gateway-db`, `gateway-core`, `gateway-policy` |
| `gateway-mcp` | Model Context Protocol server + client aggregator | `gateway-db`, `gateway-auth`, `gateway-policy` |
| `gateway-audit` | Async buffered audit writer + HMAC-signed webhooks + DLQ + `llm_log_write_errors_total` counter | `gateway-db`, `gateway-telemetry` |
| `gateway-telemetry` | OTLP traces + Prometheus metrics (incl. `circuit_breaker_open`, `llm_log_write_errors_total`) + W3C propagation | `gateway-auth`, `gateway-tenant` |
| `gateway-server` | Binary: wires all crates, CLI, background workers | all of the above |
| `gateway-tests` | Integration test suite (104 tests) | — |

## Deployment Modes (Dual-Delivery Model)

Sentinel Gateway supports three delivery modes, controlled by build-time features and runtime configuration.

### 1. Community Edition *(Open Source, Single Tenant)*
- **Build:** `cargo build` (default)
- **Status:** Unrestricted for local use.
- **Limits:** Single tenant (`local`), no multi-tenancy, no enterprise features (SSO, gRPC, etc. are stripped at compile-time).
- **Licensing:** No license required.

### 2. PaaS Edition *(Developer/Self-Hosted, All-Features)*
- **Build:** `cargo build --features saas`
- **Config:** `DEPLOYMENT_MODE=paas`, `GATEWAY__SERVER__DEVELOPER_SECRET` must match hash of `sentinel-paas:{instance_id}`.
- **Status:** Unlocks **all enterprise features** without a license server.
- **Purpose:** For developers and enterprise self-hosted environments where outbound connectivity to Licencia is not possible.

### 3. SaaS / Platform Mode *(Multi-Tenant, Licensed)*
- **Build:** `cargo build --features saas`
- **Config:** `DEPLOYMENT_MODE=platform`, `LICENCIA_URL` + `LICENCIA_API_KEY` required.
- **Status:** Per-tenant licensing. Features are gated by the plan assigned to the tenant.
- **Licensing:**
  - **Verification:** 3-tier cache (DashMap L1 → Redis L2 → Postgres L3).
  - **Resolution:** `require_feature(state, auth, feature)` checks the tenant's specific plan in real-time (~1µs overhead).
  - **Fallbacks:** Defaults to `Community` plan if license is invalid or unavailable.

## Request Lifecycle

### REST proxy (fallback route)
1. TLS check (reject with 426 if required)
2. Telemetry span created with `tenant_id`, `user_id`, `method`, `path`
3. Tenant resolved (header → JWT → subdomain → default)
4. Optional auth (may be anonymous for public routes)
5. Route matched by `path_pattern` prefix (tenant-scoped)
6. Path rewriting (strip_prefix + regex rules)
7. Policy engine: IP filter → body size → GraphQL depth → budget → rate limit
8. `GatewayEngine`:
   - Filters out backends with open circuit breakers
   - Load-balances among remaining (round-robin / weighted / least-connections / inference-aware)
   - Forwards via pooled HTTP client (256 idle conns/host, TCP keepalive)
   - Records success/failure for circuit breaker + passive health tracking
   - Falls back to next backend on failure (tries up to 2)
9. Response streamed back with status, headers, request ID

### LLM chat completion
1–4 as REST, plus:
5. Model name extracted; `"auto"` triggers complexity-based smart routing
6. `LlmRouter.select(model)` resolves alias → picks provider
7. Semantic cache check (skip if streaming or temperature > 0.5)
8. PII detection (detect/redact/block based on per-tenant mode)
9. Token count via `tiktoken-rs` for budget pre-check
10. Request adapted to provider format (OpenAI ↔ Anthropic ↔ Gemini ↔ Ollama)
11. W3C trace context injected
12. Forward → adapt response back to OpenAI format → count output tokens
13. Cost calculated, metrics recorded, usage row written (fire-and-forget)

## Key Design Decisions

| Decision | Rationale |
|---------|-----------|
| Modular monolith, not microservices | Skill rule: start monolith unless team already operates multiple services. 12 domain crates give clean boundaries without deployment complexity. |
| Short JWT + opaque refresh + JTI blacklist | Revocable within access TTL (15 min); stolen JWTs invalidated via blacklist lookup. |
| Argon2id for passwords | Memory-hard, GPU-resistant. Salted per-hash. |
| PostgreSQL table partitioning on `audit_logs` + `usage_records` | Time-series data grows linearly — partitioning by month keeps query planner fast. Auto-creation function creates 3 months ahead. |
| Partial indexes | `WHERE is_active=true` indexes skip soft-deleted rows. Saves I/O on the hot path. |
| Cursor pagination on large tables | OFFSET scans and discards rows. Composite `(created_at DESC, id DESC)` cursor is O(log n). |
| Redis required when `replicas > 1` | In-memory token buckets are per-instance — multi-replica deployments get inconsistent rate limiting **and budget tracking** without Redis. Fatal startup error enforces this for rate limiting; budget enforcer warns and falls back gracefully. |
| EVALSHA for Redis Lua rate-limit scripts | Scripts pre-loaded at startup via `SCRIPT LOAD`; all calls use `EVALSHA` with `EVAL` fallback on `NOSCRIPT`. Eliminates ~80% of per-call Redis bandwidth overhead under load. |
| Redis INCRBYFLOAT for budget tracking | Atomic float increment is the only correct primitive for cross-replica cost accumulation. Period TTLs are set on every increment so stale keys self-expire. |
| Field encryption (ChaCha20-Poly1305) for backend credentials | Credentials at rest must be encrypted per security baseline. `encryption_key` required in platform mode. |
| `pg_advisory_xact_lock` on startup migrations | Prevents race when multiple replicas start simultaneously. Only one runs migrations. |
| SQLx `log_statements` → `tracing` bridge | Unified observability — DB queries appear in OTel traces and get filtered by `log_queries` config. |
| MCP dual-role (server + client) | Gateway both exposes aggregated tools to AI agents AND consumes upstream MCP servers. Tools namespaced `{server}__{tool}` to avoid collisions. |

## Scaling Thresholds

Per [backend-architect skill](../backend/crates/gateway-server/src/main.rs) recommendations:

| Bottleneck | Threshold | Mitigation |
|-----------|-----------|------------|
| PostgreSQL writes | ~5k QPS | Add read replicas first (audit/usage queries), then shard by `tenant_id` at 5TB |
| PostgreSQL connections | 200 | PgBouncer — **now shipped** in dev compose (`pgbouncer:6432`) and prod overlay. Migrator still connects directly (DDL incompatible with transaction-mode pooling). |
| Single gateway replica | 10k QPS (CPU-bound) | Horizontal scale; requires `GATEWAY__REPLICAS>1` + Redis |
| Webhook delivery | 1k events/s | DLQ absorbs failures; increase `webhook_max_retries` or dedicate workers |
| Rate limiter (in-memory) | Single replica only | Redis backend auto-selected when `GATEWAY__REDIS__URL` set; uses EVALSHA (pre-loaded at startup) |
| Budget enforcer (in-memory) | Single replica only | Redis INCRBYFLOAT backend auto-selected when `GATEWAY__REDIS__URL` set; cross-replica spend is consistent |

## Background Workers

Spawned at startup in `gateway-server`:

| Worker | Interval | Purpose |
|--------|----------|---------|
| Active health checker | 30s | Probe `/health` on each backend, update `HealthChecker` cache |
| Token blacklist cleanup | 60s | Remove expired JTIs from DashMap |
| API key cache cleanup | 60s | Remove expired entries |
| Rate limiter cleanup | 5min | Remove idle in-memory token buckets (no-op for Redis backend) |
| Budget stale period cleanup | 1h | Remove old daily/weekly/monthly period entries (no-op for Redis backend — keys self-expire via TTL) |
| License heartbeat | 1h | Re-validate against platform (online mode only) |

## Database Schema

13 migrations, 10 core tables + time-partitioned tables:

- `tenants`, `users`, `api_keys` (hot path)
- `backends`, `routes`, `settings`
- `audit_logs` *(partitioned by month)*
- `usage_records` *(partitioned by month)*
- `licenses`, `webhook_endpoints`, `webhook_failures` *(DLQ)*
- `mcp_servers` *(upstream MCP registry)*

See [migrations/](../backend/migrations/) and `013_production_hardening.sql` for partial indexes + partition functions.
