# Sentinel Gateway — Configuration Reference

All settings can be configured via either:
- **TOML file** at `config/gateway.toml` (loaded on startup)
- **Environment variables** with prefix `GATEWAY__` using `__` as separator

Env vars always override TOML. Unset values fall back to the built-in defaults in `crates/gateway-server/src/config.rs`.

## Examples

TOML:
```toml
[server]
port = 9000
require_tls = true

[database]
url = "postgres://user:pass@db:5432/app"
auto_migrate = false
```

Equivalent env:
```bash
GATEWAY__SERVER__PORT=9000
GATEWAY__SERVER__REQUIRE_TLS=true
GATEWAY__DATABASE__URL=postgres://user:pass@db:5432/app
GATEWAY__DATABASE__AUTO_MIGRATE=false
```

---

## `[server]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | string | `0.0.0.0` | Bind address |
| `port` | u16 | `8080` | Bind port |
| `deployment_mode` | string | `local` | `"local"` (single-tenant), `"paas"` (all-features unlocked via secret), or `"platform"` (multi-tenant SaaS) |
| `saas_mode` | bool | `false` | Internal flag. Set to `true` when multiple tenants are allowed. |
| `developer_secret` | string? | `None` | Used in `paas` mode. Must match SHA-256 of `sentinel-paas:{instance_id}` to unlock features. |
| `max_body_size` | usize | `10485760` | Request body limit (bytes, 10 MB default) |
| `instance_id` | string? | `None` | UUID; auto-generated on first run and stored in settings |
| `instance_name` | string? | `None` | Human-readable name for this deployment |
| `encryption_key` | string? | `None` | 64-hex-char key for field encryption (ChaCha20-Poly1305). **Required in platform mode.** Generate: `openssl rand -hex 32` |
| `cors_allow_all` | bool | `true` | Dev-friendly default. **Must be `false` in platform mode.** |
| `cors_origins` | [string] | `[]` | Explicit origins when `cors_allow_all=false` |
| `require_tls` | bool | `false` | Reject plaintext HTTP with `426 Upgrade Required`. Trusts `X-Forwarded-Proto` from trusted proxy. |
| `trust_forwarded_proto` | bool | `true` | Trust `X-Forwarded-Proto` header (set `false` if no reverse proxy) |

## `[database]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `url` | string | `postgres://sentinel:sentinel@localhost:5432/sentinel_gateway` | Connection URL. In dev compose the backend connects through PgBouncer (`pgbouncer:6432`); the migrator connects directly to Postgres (`db:5432`). |
| `max_connections` | u32 | `20` | SQLx pool max per replica. With PgBouncer in front this maps to PgBouncer's `DEFAULT_POOL_SIZE`. |
| `auto_migrate` | bool | `true` | Run migrations at startup with `pg_advisory_lock`. **Set `false` in production** and use a dedicated migrator job. The migrator connects directly to Postgres, bypassing PgBouncer (transaction mode is incompatible with advisory locks). |
| `enable_query_stats` | bool | `false` | Enable `pg_stat_statements` + `/admin/slow-queries` endpoint. Requires superuser or managed DB with extension preinstalled. |
| `log_queries` | bool | `false` | Log all SQL at DEBUG. Slow queries (>200ms) always logged at WARN regardless. |

## `[auth]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `jwt_private_key_path` | string | `keys/private.pem` | RSA private key for signing JWTs |
| `jwt_public_key_path` | string | `keys/public.pem` | RSA public key for verifying JWTs |
| `access_token_ttl_minutes` | i64 | `15` | Short TTL keeps revocation exposure small |
| `refresh_token_ttl_days` | i64 | `7` | Longer but revocable via blacklist on rotation |
| `max_failed_logins` | i32 | `5` | Per-user lockout threshold (DB-tracked) |
| `lockout_duration_minutes` | i64 | `15` | How long an account stays locked |
| `api_key_cache_ttl_secs` | u64 | `300` | In-memory API key cache TTL |

Generate keys:
```bash
gateway-server generate-keys --output-dir keys
```

## `[telemetry]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `otlp_endpoint` | string? | `None` | OTLP gRPC endpoint, e.g. `http://otel-collector:4317` |
| `service_name` | string | `sentinel-gateway` | OTel resource attribute |
| `log_level` | string | `info` | Standard `tracing_subscriber::EnvFilter` syntax (`info,gateway_server=debug`) |
| `prometheus_enabled` | bool | `true` | Expose `/metrics` endpoint |

## `[proxy]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `default_timeout_ms` | u64 | `30000` | Per-request timeout |
| `connect_timeout_ms` | u64 | `5000` | TCP connect timeout |
| `max_retries` | u32 | `3` | Retry attempts on 5xx + connection errors |
| `health_check_interval_secs` | u64 | `30` | Active health-check period |
| `circuit_breaker_threshold` | u32 | `5` | Failures before opening |
| `circuit_breaker_recovery_secs` | u64 | `60` | Half-open after this delay |
| `pool_idle_timeout_secs` | u64 | `90` | Connection idle timeout |
| `pool_max_idle_per_host` | usize | `256` | Max idle connections per backend host |
| `max_body_size` | usize | `10485760` | Proxy-forwarded body limit |
| `graphql_max_depth` | u32 | `10` | GraphQL query depth ceiling |
| `websocket_enabled` | bool | `true` | Enable WebSocket upgrade detection + bidirectional relay |
| `lb_strategy` | string | `round_robin` | `round_robin` / `weighted` / `least_connections` / `inference_aware` |

## `[license]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `license_key` | string? | `None` | Instance-wide offline license key. |
| `public_key_path` | string? | `None` | RSA public key for instance license verification. |
| `grace_period_days` | i64 | `7` | How long after expiry before features degrade |

## `[platform]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `licencia_url` | string? | `None` | Licencia server URL for online activation (SaaS mode). |
| `licencia_api_key` | string? | `None` | API key for authenticating against Licencia server. |
| `sync_interval_secs` | u64 | `3600` | Platform sync frequency |
| `auto_sync` | bool | `false` | Background auto-sync at interval |

## `[redis]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `url` | string? | `None` | e.g. `redis://localhost:6379`. **Required when `GATEWAY__REPLICAS > 1`**. When set, both the rate limiter **and the budget enforcer** use Redis backends. Rate-limit Lua scripts are pre-loaded via `SCRIPT LOAD` at startup (EVALSHA on subsequent calls). |

> **Tip:** in dev, `GATEWAY__REDIS__URL` is optional. In production with `replicas > 1`, omitting it causes a fatal startup error (rate limiter) and silent per-replica budget drift (budget enforcer).

## Standalone env vars (not in TOML)

| Variable | Default | Description |
|----------|---------|-------------|
| `GATEWAY__REPLICAS` | `1` | Hint to the server about replica count. If >1 without Redis → fatal startup error |
| `RUST_LOG` | — | Standard Rust log filter (overrides `telemetry.log_level` if set) |
| `DATABASE_URL` | — | Used by `sqlx-cli` and some tooling |
| `TEST_DATABASE_URL` | — | Used by `gateway-tests` crate |

## Production Config Validation

When `deployment_mode = "platform"`, startup will **fail fast** if:
- `encryption_key` is missing or not 64 hex chars
- `cors_allow_all = true` with empty `cors_origins`

It will **warn** (but not fail) if:
- `require_tls = false`
- `auto_migrate = true` with multiple replicas

## Config Profiles

Suggested TOML overlays for common environments:

### Dev (default)
Everything out-of-the-box just works. In-memory rate limiter, no TLS, local tenant.

### Staging
```toml
[server]
deployment_mode = "platform"
cors_allow_all = false
cors_origins = ["https://staging.example.com"]
require_tls = true
encryption_key = "$STAGING_ENCRYPTION_KEY"

[database]
auto_migrate = false
enable_query_stats = true

[redis]
url = "redis://redis:6379"

[telemetry]
otlp_endpoint = "http://otel:4317"
log_level = "info,gateway_server=debug"
```

### Production
```toml
[server]
deployment_mode = "platform"
cors_allow_all = false
cors_origins = ["https://app.example.com"]
require_tls = true
encryption_key = "$PROD_ENCRYPTION_KEY"

[database]
max_connections = 40
auto_migrate = false
log_queries = false
enable_query_stats = true

[proxy]
pool_max_idle_per_host = 512
lb_strategy = "inference_aware"

[redis]
url = "redis://redis-primary:6379"

[telemetry]
otlp_endpoint = "http://otel-collector:4317"
log_level = "warn,gateway_server=info"
```

With `GATEWAY__REPLICAS=3` in the pod env.
