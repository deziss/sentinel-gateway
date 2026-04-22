# Changelog

All notable changes. Semver once we tag `v1.0.0`.

## [Unreleased] — Production Readiness (P0/P1)

### Added — Infrastructure: PgBouncer in dev compose

- **`docker-compose.yml`**: added `pgbouncer` service (edoburu/pgbouncer, transaction pool mode, port 6432).
  - `backend` now connects via `pgbouncer:6432` instead of `db:5432`.
  - `migrator` retains a direct connection to `db:5432` (DDL is incompatible with PgBouncer transaction mode).
  - `DEFAULT_POOL_SIZE=20`, `MAX_CLIENT_CONN=100`, health-checked by `backend`.
  - Matches the production overlay (`docker-compose.prod.yml`) which was already running PgBouncer.

### Added — Policy: Redis-backed `BudgetEnforcer`

- **`gateway-policy/src/budget.rs`**: `BudgetEnforcer` refactored from a single in-memory `DashMap` to a dual-backend enum:
  - `InMemory` — single-replica default, uses `DashMap<BudgetKey, f64>`.
  - `Redis` — multi-replica safe; uses `INCRBYFLOAT` (atomic) + `EXPIRE` (auto-expiring per budget period).
  - `BudgetEnforcer::new_redis(client)` constructor.
  - Budget period TTLs: Daily=86400s, Weekly=604800s, Monthly=2678400s.
- **`gateway-policy/src/engine.rs`**: `check_budget` and `record_usage` are now async. `record_usage` is wrapped in `tokio::spawn` fire-and-forget to keep the hot path non-blocking.
- **`gateway-server/src/main.rs`**: `BudgetEnforcer::new_redis` is instantiated at startup when `GATEWAY__REDIS__URL` is configured (with a 5s timeout and graceful fallback).

### Added — Performance: Redis Lua script pre-loading (EVALSHA)

- **`gateway-policy/src/rate_limiter.rs`**: `RateLimiter::preload_scripts()` method added.
  - On startup, both rate-check (`LUA_CHECK`) and consume (`LUA_CONSUME`) scripts are uploaded via `SCRIPT LOAD`, returning SHA1 digests stored in `ScriptShas`.
  - All Redis calls use `EVALSHA` first; on `NOSCRIPT` error (e.g. Redis restart) they transparently fall back to inline `EVAL`.
  - Reduces per-call Redis bandwidth by ~80% under load.
- **`gateway-server/src/main.rs`**: `rate_limiter.preload_scripts().await` called immediately after Redis connects.

### Added — Observability: LLM log write-error counter

- **`gateway-audit/src/llm_log_service.rs`**: `llm_log_write_errors_total{error_kind}` Prometheus counter registered and incremented on:
  - `channel_full` — fire-and-forget mpsc buffer overflow (request-path drop).
  - `db_timeout` — batch insert exceeded timeout.
  - `db_error` — generic DB error during batch flush.
  - Flush failures are now logged at `ERROR` level with record count, making silent data loss observable.
- **`gateway-audit/Cargo.toml`**: added `prometheus`, `once_cell`, `gateway-telemetry` dependencies.

### Added — Observability: Circuit-breaker gauge + new metrics helpers

- **`gateway-telemetry/src/metrics.rs`**:
  - `circuit_breaker_open{backend_id, tenant_id}` GaugeVec (1=open, 0=closed/half-open).
  - `llm_log_write_errors_total{error_kind}` CounterVec (mirrors audit crate's counter; registered in shared REGISTRY).
  - `Metrics::set_circuit_breaker_state(backend_id, tenant_id, open)` helper method.
  - `Metrics::record_llm_log_write_error(error_kind)` helper method.

### Added — Observability: Prometheus SLO recording rules

- **`deploy/prometheus/rules.yml`** (new file): SLI recording rules covering:
  - Success ratio: `sli:http_request_success_ratio:{5m,1h}`, `sli:http_request_success_ratio_by_tenant:5m`
  - Latency: `sli:http_request_duration_{p95,p99}:5m`, per-tenant proxy equivalents
  - Error-budget burn rate: 1h and 6h windows (Google SRE Workbook multi-window pattern)
  - Backend health aggregates: ratio and all-unhealthy flag per tenant
  - LLM telemetry: cost/token rates per tenant, log write error rate
- **`deploy/prometheus/prometheus.yml`**: registered `rules.yml` in `rule_files:`.

### Changed

- `BudgetEnforcer::check()` and `record_usage()` are now `async fn` (breaking change for direct callers — only `PolicyEngine` calls them).
- `PolicyEngine` updated to `await` budget calls; no public API change.
- Scaling table in `docs/architecture.md` updated: PgBouncer listed as shipped, budget enforcer added as in-memory bottleneck.
- PgBouncer runbook in `docs/operations.md` updated from "add PgBouncer" to reflect it is now a first-class service.

---



Commercial plan tiers + SSO + feedback + organizations + data-lake exports.

### Added — Plan tiers (Open Source / Professional / Enterprise)

- **Three-tier feature matrix** (`gateway-license/src/features.rs`) — exactly matches the published pricing matrix:
  - `Plan::Community` (OSS, self-hosted, unlimited) — core gateway routing only: Universal API, Fallbacks, Loadbalancing, Conditional Routing, Retries, Timeouts. Everything else off.
  - `Plan::Professional` (100K req/month, 30-day retention) — full observability (except FinOps), simple + semantic cache, prompt management (unlimited templates + versioning + playground), deterministic/partner guardrails + PII redaction, RBAC, team management.
  - `Plan::Enterprise` (unlimited, custom retention) — everything Pro + FinOps dashboard, SSO (Okta/Keycloak/Google/GitHub/Microsoft), audit logs, SCIM, JWT auth, BYOK, datalake exports, org management, compliance (SOC2/GDPR/BAA), VPC/private tenancy.
- **`Feature` enum + `min_plan()` + `Plan::meets()`** — single source of truth for gating decisions, 30+ named features.
- **`GET /api/v1/license/features`** — returns full flag set; frontend drives upsell UI from this.
- **Feature-gate helper** (`gateway-server/src/handlers/feature_gate.rs`) — `require_feature(&state, Feature::X).await?` returns `402 Payment Required` with `{error: {code: "feature_gated", feature, required_plan, current_plan}}`.
- **Gated handlers** — Feedback, SSO (authorize/callback + CRUD), Organizations enforce their feature flags.

### Added — SSO (OAuth2 / OIDC, Enterprise-gated)

- **5 providers** (`gateway-auth/src/sso.rs`) — Keycloak, Okta, Google, GitHub, Microsoft Entra. Shared `OidcProvider` + dedicated `GithubProvider` (non-OIDC).
- **Migration 019** — `sso_providers`, `sso_identities`, `sso_auth_states` (CSRF + PKCE verifier storage).
- **Public endpoints** — `GET /auth/sso/:slug/authorize` + `GET /auth/sso/:slug/callback` with PKCE S256, single-use state (atomic DELETE RETURNING), nonce, auto-provisioning.
- **Admin endpoints** (TenantAdmin+) — `GET/POST /sso/providers`, `DELETE /sso/providers/:id`.
- Audit logging on every SSO login + provider config change.

### Added — LLM Feedback (Pro-gated)

- **Migration 020** — `llm_feedback` (rating ∈ {-1, 0, 1}, comment, JSONB metadata, indexed by tenant + log_id + request_id).
- **3 endpoints** — `POST /feedback` (submit), `GET /feedback` (list), `GET /feedback/stats` (aggregate positive/negative/ratio over N days).
- Keyed to either `llm_log_id` or externally-supplied `request_id`.

### Added — Organizations (Enterprise-gated)

- **Migration 020** — `organizations` (tenant-of-tenants) + `tenants.organization_id` FK. Enables parent-child grouping for billing, cross-tenant views, and enterprise accounts with multiple environments.
- **6 endpoints** — CRUD + tenant assignment + tenant listing.

### Added — Plugin framework

- **New crate `gateway-plugin`** — trait-based request/response lifecycle hooks:
  - `Plugin` trait with `before_request` / `after_response` / `on_error`.
  - 5 plugin kinds: Input, Output, Guardrail, Observer, Auth. Ordered pipelines (kind → priority → name).
  - `PluginDecision::{Continue, Modified, Block, Respond}` — terminal decisions short-circuit the pipeline.
  - `PluginContext` with free-form metadata bag for inter-plugin state sharing.
  - `PluginRegistry` with hot-swap (`register` / `unregister`), disabled-plugin skipping, error isolation (errored plugins don't halt the pipeline).

### Added — Virtual keys activation (P0)

- **3rd auth method** — `vk_*` prefix routes to virtual-key authentication alongside JWT + `sg_*` API keys.
- `AuthMethod::VirtualKey { vkey_id, backend_id, team_id, allowed_models, rate_limit_rpm, budget_daily, budget_monthly }`.
- Per-key pinning to a backend + per-key rate/budget policy.
- Fire-and-forget `touch_used()` updates `last_used_at`.

### Added — Write-behind LLM logging

- **`gateway-audit/src/llm_log_service.rs`** — async mpsc-buffered batch-insert (200 entries or 2s flush). Drops silently on overflow rather than blocking the LLM request path.
- All successful LLM requests captured with redacted request + response for search/replay.
- **Error observability added (P0-4):** flush failures increment `llm_log_write_errors_total{error_kind}` counter; errors are logged at `ERROR` with record count and data-loss warning.

### Added — Per-tenant pricing overrides

- `CostCalculator::calculate_with_override()` — applies per-model input/output price overrides + markup multiplier.
- `tenant_pricing` repo wired into LLM handler; falls back to default table when no override.

### Added — Simple cache + privacy mode

- **Per-tenant scoped cache** (`gateway-llm/src/privacy.rs::tenant_cache_key`) — namespaces fingerprint with `tenant_id:` so tenant A can't read tenant B's cache.
- **Privacy mode** — `privacy_mode=true` setting redacts message content (request + response) in observability exports and DB logs while preserving role, tokens, cost, and other metadata.
- Gated by tenant setting `llm_cache_enabled`.

### Added — Data-lake exports (Enterprise-gated)

- **`gateway-audit/src/data_lake.rs`** — NDJSON spooler supporting 3 destinations:
  - `file://` — local directory with date partitioning (`dir/YYYY-MM-DD/`)
  - `s3://bucket/prefix/` — uploads via `aws s3 cp`
  - `gs://bucket/prefix/` — uploads via `gsutil cp`
- 5-minute rotation default (configurable via `DATA_LAKE_ROTATE_SECS`).
- Bounded 100K in-memory queue; drops silently when full (non-blocking).

### Added — Retention worker

- `spawn_retention_worker` — daily cleanup deletes `llm_logs` older than per-tenant `llm_log_retention_days` setting, fallback to `LLM_LOG_RETENTION_DAYS` env (default 90).

### Added — Frontend pages + plan-aware UI

- **`/feedback`** — feedback stats + list (thumbs up/down/ratio over configurable window).
- **`/sso-providers`** — SSO provider CRUD for 5 OAuth2 providers.
- **`/organizations`** — organization CRUD (SuperAdmin).
- **`/billing`** — full feature matrix with plan cards, upgrade CTAs.
- **`<PlanBadge>`** — current plan badge in header.
- **`<FeatureGate>`** — upsell component for gated features.
- **`usePlan()` hook** — caches `/license/features` for 5 min; `has(flag)` + `meets(tier)` helpers.
- Sidebar nav auto-hides gated items based on current plan.

### Added — Tests

- Backend: **204 tests** (up from 138) — plugin framework (registry ordering, Block/Respond/Continue flow, disabled-plugin skipping, error isolation), privacy redaction (chat/completions/embeddings/multimodal), `cost::calculate_with_override` (overrides + markup + clamping + partial override fallback), SSO (PKCE determinism, provider defaults, authorize URL shape, GitHub non-OIDC exemption), updated `Feature` / `Plan::meets` consistency check across all 28 features.
- Frontend: **58 tests** (up from 0) — vitest + @testing-library/react + jsdom. Covers: fp-validate, api helpers (token store + error mapping + SSO URL builder), JWT decode, Feedback/Organizations page smoke tests, FeatureGate gating behavior (community/pro/enterprise), `planMeets` tier logic.

### Fixed

- **Migration 018 + 019** — replaced Rust-style `///` doc comments with SQL `--` comments (PostgreSQL rejected the `///` tokens).
- **Frontend Docker build** — excluded test files from `tsconfig.app.json` production build so test-only types aren't required for the Nginx image.

### Changed

- **`FeatureFlags::for_plan(Plan::Community)`** — removed overly-generous OSS defaults. Community tier is now strictly core gateway routing; observability, prompts, guardrails, webhooks, IP filtering, and budget enforcement moved to Pro+ (as per published matrix).
- **Community monthly quota** — unlimited (self-hosted). **Pro** — 100K/month. **Enterprise** — custom.
- **Retention** — Community: 0 (no-op), Pro: 30 days, Enterprise: unlimited/custom.

## [Unreleased]

### Added — P2 moat features

- **CEL token-cost rate limiting** (`gateway-policy/src/cel_cost.rs`) — programmable rate-limit cost functions:
  - `CostExpression::parse()` compiles CEL expressions once, evaluates in ~1µs per request
  - Variables: `input`, `output`, `cached`, `cache_creation`, `reasoning`, `total`, `model`, `tenant` + OpenAI aliases (`prompt_tokens`, etc.)
  - `CelRateLimitRegistry` caches parsed programs; integrate via `CelRateLimit::consume()`
  - 8 unit tests covering weighted cost, cached-token discounts, reasoning penalties, model-aware conditionals, registry caching
- **Semantic Policy Engine** (`gateway-policy/src/semantic.rs`) — block/flag/require actions based on prompt meaning:
  - `Embedder` trait with two implementations: `HashEmbedder` (zero-deps char-trigram hashing trick), `HttpEmbedder` (any OpenAI-compatible `/v1/embeddings`)
  - `SemanticPolicyEngine` — loads topic references, embeds once, matches via cosine similarity at request time
  - `SemanticGuardrail` plugs into the existing `GuardrailPipeline`
  - Embedding cache with bulk-evict-on-full
  - 9 unit tests covering cosine, determinism, L2 normalization, topic matching, thresholds, caching, block action
- **Inference-aware routing** (`gateway-core/src/inference_metrics.rs`) — vLLM/TGI/SGLang queue-depth + KV-cache-aware routing:
  - `InferenceMetricsCache` with TTL-based freshness
  - `scrape_once` parses Prometheus text format for vLLM (`vllm:num_requests_waiting`, `vllm:gpu_cache_usage_perc`, `vllm:gpu_prefix_cache_hit_rate`), TGI, SGLang
  - New `LbStrategy::InferenceAware` scores backends by `(queue + running) - cache_bonus - prefix_bonus`; falls back to least-connections when metrics are stale
  - Background scraper task in `main.rs` polls at `health_check_interval_secs`
  - 6 unit tests covering vLLM/TGI parsing, routing priority, staleness handling

### Added — P1 roadmap (round 3)

- **Prompt Management & Versioning** (migrations 015, `gateway-db/src/models/prompt.rs`, `handlers/prompts.rs`):
  - `prompts` table (versioned content + variables + model_prefs + metadata)
  - `prompt_deployments` (label → version mapping)
  - 7 API endpoints: create (auto-increments version), list names, list versions, get version, deploy, list deployments, resolve (with variable rendering)
  - Chat handler extension: `prompt_ref: {name, label, variables}` in `/v1/chat/completions` resolves and injects as system message with `default_model` + `model_prefs` merging
  - Template rendering via `{{var_name}}` substitution (undefined vars preserved)
  - Frontend `Prompts.tsx` — sidebar + versions/deployments tabs, create/deploy/test-render/delete dialogs
- **Guardrails Framework** (migrations 017, `gateway-policy/src/guardrails.rs`):
  - Trait-based `Guardrail { check(&self, ctx) -> GuardrailOutcome }`
  - Built-ins: `RegexGuardrail`, `LengthGuardrail`, `JsonSchemaGuardrail`, PII regex expansion
  - `GuardrailPipeline` chains multiple guards with modification propagation and first-block short-circuit
  - Stages: `pre_call`, `post_call`, `logging_only`; modes: `block`, `redact`, `flag`
  - `guardrail_rules` table for per-tenant persistence + `guardrails_build.rs` translator
  - 6 REST endpoints + `/guardrails/test` for pipeline validation
  - Frontend `Guardrails.tsx` — rule table with inline toggle, live test dialog, kind-specific config hints
  - 5 unit tests on pipeline behavior + 4 on rule builder
- **Provider Catalog Expansion** (migration 014): added Mistral, Cohere, DeepSeek, Groq, Together, Perplexity, Fireworks (all OpenAI-compatible). Total supported providers: 17.
- **Data-Policy Filtering** (migration 016): `backends.data_policy` enum (`standard` / `no_retention` / `no_training` / `strict`), ordered `PartialOrd`. Enables compliance-driven routing (HIPAA → strict only).
- **Observability Export — optional Langfuse + Helicone forwarding** (`gateway-server/src/observability_export.rs`):
  - Opt-in; disabled by default
  - Bounded mpsc queue (1000 events default), fire-and-forget
  - Concurrent fan-out via `tokio::join!` to both destinations
  - Config: `GATEWAY__OBSERVABILITY__LANGFUSE__*` and `GATEWAY__OBSERVABILITY__HELICONE__*`
  - Wired into `chat_completions` handler

### Added — Security hardening

- **ChaCha20-Poly1305 field encryption** (`gateway-core/src/crypto.rs`) — `FieldEncryptor` with `encrypt` / `decrypt`, random nonces, inline tests
- **Input validation everywhere** — `validator` crate derives on all request structs (login, invite, create-backend, create-key, webhook, tenant, sync, guardrail, prompt)
- **CORS + security headers** — configurable CORS (`permissive` default for dev, strict for prod), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `X-XSS-Protection`, `Strict-Transport-Security`, `Content-Security-Policy`, `Permissions-Policy`
- **Secret redaction** — telemetry middleware strips `Authorization`, `X-API-Key`, `Cookie`
- **Real client-IP extraction** — `X-Forwarded-For` → `X-Real-IP` → `ConnectInfo` fallback, used in all audit events (replacing hardcoded `"127.0.0.1"`)
- **Frontend error sanitization** — `sanitizeApiError()` maps status codes to user-safe messages

### Added — MCP Gateway

- **New `gateway-mcp` crate** — dual-role Model Context Protocol proxy:
  - Protocol 2025-06-18, JSON-RPC 2.0, Streamable HTTP transport
  - `McpServer` (agent-facing) — handles `initialize`, `tools/*`, `resources/*`, `prompts/*`, `ping`, notifications
  - `McpClient` (upstream-facing) — connects to any Streamable-HTTP MCP server, runs handshake, discovers primitives
  - `McpRegistry` — aggregates tools with namespacing (`{backend}__{tool}`), resources with URI namespacing (`mcp://{backend}/{uri}`)
  - `SessionStore` — per-client session tracking with TTL
- **5 new REST endpoints** under `/api/v1/mcp/*`:
  - JSON-RPC proxy (`POST /mcp`), server CRUD, refresh discovery, list aggregated tools
- **Frontend `McpServers.tsx`** — servers tab with health/tool/resource/prompt cards, aggregated tools tab with namespaced view, register/refresh/remove actions
- **Migration 012** — `mcp_servers` table for persistent server configs with encrypted auth credentials

### Added — Task 16: Kubernetes Helm

- Fixed secret encoding bug (`| b64enc` on JWT keys)
- Added RBAC (Role + RoleBinding) with narrow permissions on own ConfigMap/Secret
- Added `NOTES.txt` with post-install instructions
- Backend deployment: preStop hook, init-container resource limits
- Frontend deployment: pod anti-affinity, PDB (min 1 available), startup probe, graceful shutdown, nginx cache emptyDir volumes, rolling update strategy
- Services: configurable NodePort + annotations on backend and frontend
- PDB: added frontend PDB
- NetworkPolicy: frontend policy (ingress from nginx, egress to backend only), backend allows ingress from frontend + Prometheus scraping, egress to OTel Collector
- `values.yaml`: service annotations, NodePort, SaaS mode config, OTLP endpoint, ingress `/v1` path, HPA custom metrics example (commented)

### Added — Task 15: Docker + Compose

- **Backend Dockerfile**: 4-stage cargo-chef build (chef → planner → builder → runtime); non-root user; built-in `HEALTHCHECK`
- **`docker-compose.yml`**: backend health check with `service_healthy` condition, OTel Collector service, `keygen` init container for automatic RSA key generation, `service_completed_successfully` for migrator, Redis memory limits
- **`docker-compose.prod.yml`** — new production overlay:
  - PgBouncer (edoburu/pgbouncer:1.23.1) with transaction pool mode
  - Resource limits on all services
  - Required secret env vars (`${POSTGRES_PASSWORD:?Set POSTGRES_PASSWORD}`) — fails fast if missing
  - No host ports on db/redis
  - JSON file log rotation
  - `restart: unless-stopped`
- **nginx.conf** upgrades: gzip, security headers, 1-year static asset caching, SSE streaming for `/v1/`, WebSocket support for `/ws/`, split timeout configs per route group
- **`.dockerignore`** for backend and frontend
- **`deploy/docker/otel-collector.yaml`** — OTLP receiver → Prometheus + debug exporters
- **Root `.env.example`** — all env vars for docker-compose

### Added — Task 14: LLM Playground & Analytics

- **API layer additions**: `listModels`, `chatCompletion`, `streamChatCompletion` (SSE via fetch + ReadableStream), `createEmbedding`, full types
- **LlmPlayground.tsx**: dynamic model list from backends, wired temperature/max_tokens/top_p, system prompt textarea, streaming toggle with real-time token rendering, prompt/completion/total token breakdown, clear-chat confirmation dialog, new Embeddings tab
- **LlmAnalytics.tsx**: replaced hardcoded data with real API via `getUsageSummary()` with 30s auto-refetch; input vs output token breakdown with visual progress bars; active LLM providers table
- **LlmCatalog.tsx**: functional search (name/provider/endpoint), provider filter dropdown, status filter, view-details dialog with full specs

### Added — Task 13: Core Pages

- **Foundation UI components**: AlertDialog, ConfirmDialog, Textarea, Switch, Skeleton, Separator, Checkbox
- **API layer expansion**: updateBackend, revokeApiKey, updateUser, settings CRUD, webhooks CRUD, tenant management
- **Dashboard.tsx**: error state, skeleton loading, empty state, matches `UsageSummary` contract
- **Backends.tsx**: full CRUD with edit dialog, confirmation on delete, fp-ts validation, provider select
- **ApiKeys.tsx**: create with scopes/rate limits/budgets, revoke with confirmation dialog, copy-to-clipboard
- **Users.tsx**: invite with email/password validation (fp-ts), role update dialog, deactivate confirmation
- **Routes.tsx**: create dialog wired with name/path/protocol/backend selector/strip-prefix switch, delete confirmation
- **Settings.tsx**: full API integration (settings + webhooks CRUD), Tabs layout, webhook secret copy flow
- **AuditLogs.tsx**: functional search, resource-type filter, CSV export, JSON details dialog, offset-based pagination

### Added — Task 12: Frontend Setup

- Vite 6 + React 19 + TS 5.6, Tailwind CSS 4, shadcn/ui (New York / zinc)
- 9 initial shadcn components (Button, Card, Input, Dialog, Table, Toast, Badge, Label, Select)
- Zustand auth store with localStorage persistence
- Axios API client with interceptors (JWT Bearer, X-Tenant-ID header, 401 auto-refresh)
- Dashboard layout with sidebar navigation, Gateway + LLM Proxy sections
- React Query provider with QueryClient
- 11 initial pages: Login, Dashboard, Backends, ApiKeys, Users, ProxyRoutes, AuditLogs, Settings, LlmPlayground, LlmAnalytics, LlmCatalog

### Changed

- LoadBalancer: added `inference_metrics: Option<InferenceMetricsCache>` field + `.with_inference_metrics()` builder + `InferenceAware` strategy branch
- `gateway-db::Backend` struct: added `data_policy: DataPolicy` field
- `gateway-llm::ProviderType`: added 7 new variants (Mistral/Cohere/DeepSeek/Groq/Together/Perplexity/Fireworks), simplified `chat_url()` using `is_openai_compatible()`
- `AppState`: added `prompt_repo`, `guardrail_rule_repo`, `mcp_server`, `observability_exporter`

### Fixed

- Helm secret template: `jwtPrivateKey`/`jwtPublicKey` were missing `| b64enc` — keys now correctly base64-encoded
- `gateway-server::config::AppConfig::default()` now initializes the `observability` field
- Audit log client IP: was hardcoded to `"127.0.0.1"`, now properly extracted from `X-Forwarded-For` → `X-Real-IP` → `ConnectInfo`

---

## Notes on versioning

- **Backward compatibility:** API is pre-1.0 and subject to change. We keep a CHANGELOG entry for every breaking change starting with v1.0.
- **Database migrations:** every migration is forward-only. To roll back, restore from backup. Migrations are idempotent where possible.
- **Cargo lockfile:** committed — reproducible builds across environments.

---

## Test coverage

| Crate | Tests | Coverage area |
|---|---|---|
| `gateway-core` | 6 | Inference metrics parsing + routing scoring, crypto roundtrip |
| `gateway-policy` | 22 | Rate limiter, budget, guardrails (5), CEL cost (8), semantic (9) |
| `gateway-llm` | 8 | Router, cost calculator, token counter |
| `gateway-server` | 6 | Prompt template rendering, guardrails_build (4), router integration |
| `gateway-db` | 0 | Integration tests recommended — see `gateway-tests` |
| `gateway-mcp` | 5 | Protocol parsing, registry namespacing |
| **Total** | **47** unit tests passing |  |
