# Changelog

All notable changes. Semver once we tag `v1.0.0`.

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
