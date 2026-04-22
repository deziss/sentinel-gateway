# Sentinel Gateway — API Reference

**Base URL:** `https://gateway.example.com/api/v1`
**Auth:** `Authorization: Bearer <JWT>` **or** `X-API-Key: sg_<key>` **or** `Authorization: Bearer sg_<key>`

All request/response bodies are JSON unless noted. All timestamps are RFC 3339 UTC. All IDs are UUIDs.

---

## Table of Contents

- [Auth](#auth)
- [Users](#users)
- [API Keys](#api-keys)
- [Tenants](#tenants) *(SuperAdmin)*
- [Backends](#backends)
- [Routes](#routes)
- [Settings](#settings)
- [LLM (OpenAI-compatible)](#llm-openai-compatible)
- [Guardrails](#guardrails)
- [Prompts (versioning)](#prompts-versioning)
- [MCP (Model Context Protocol)](#mcp-model-context-protocol)
- [Webhooks + DLQ](#webhooks--dlq)
- [Feedback](#feedback) *(Pro+)*
- [SSO / OAuth2](#sso--oauth2) *(Enterprise)*
- [Organizations](#organizations) *(Enterprise)*
- [Audit Logs](#audit-logs)
- [Usage](#usage)
- [License](#license)
- [Platform Sync](#platform-sync)
- [Admin](#admin-superadmin)
- [Health + Metrics](#health--metrics)
- [Errors](#error-format)

---

## Auth

### `POST /auth/login`
Per-IP rate-limited to 10 req/min. Returns 429 with `Retry-After` on exceed.

**Request**
```json
{ "tenant_slug": "acme", "email": "user@acme.com", "password": "..." }
```

**Response 200**
```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "token_type": "Bearer",
  "expires_in": 900
}
```

**Errors**
- `401` invalid credentials
- `423` account locked (too many failed attempts)
- `429` rate-limited — includes `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `Retry-After`

### `POST /auth/refresh`
**Request**: `{ "refresh_token": "eyJ..." }`
Returns the same shape as `/auth/login`. Old refresh token is revoked.

### `POST /auth/logout` *(auth required)*
**Request** (optional): `{ "refresh_token": "eyJ..." }`
Revokes the caller's access token and optionally the refresh token.

---

## Users

### `GET /users` *(auth required)*
List users in the caller's tenant. Never returns `password_hash` / `mfa_secret`.
```json
{ "users": [{ "id": "...", "email": "...", "role": "user", "status": "active", ... }], "total": 12 }
```

### `POST /users` *(TenantAdmin+)*
**Request**
```json
{ "email": "new@acme.com", "password": "min-8-chars", "role": "user" }
```
Roles: `super_admin` (SuperAdmin-only), `tenant_admin`, `user`, `read_only`.
Quota-enforced against `tenant.max_users`.

### `GET /users/:id` / `PUT /users/:id` / `DELETE /users/:id`
PUT accepts `{ "role": "...", "status": "..." }`. DELETE sets `status=inactive` (soft-delete).

---

## API Keys

### `GET /api-keys` *(auth required)*

### `POST /api-keys` *(auth required, quota-enforced)*
**Request**: `{ "name": "prod-key", "scopes": ["*"] }`
**Response 201**: `{ "key": "sg_<plaintext-shown-once>", "metadata": {...} }`

The plaintext key is returned **only once**. Store it securely.

### `DELETE /api-keys/:id`
Soft-revoke. Key is immediately rejected on subsequent requests.

---

## Tenants *(SuperAdmin)*

Local mode returns 403: `"Multi-tenant not available in Community edition"`.
Platform mode requires `features.multi_tenant=true` (Professional+).

### `GET /tenants` / `POST /tenants` / `GET|PUT|DELETE /tenants/:id`
Standard CRUD. `POST` body:
```json
{
  "name": "Acme Corp",
  "slug": "acme",
  "plan": "community",
  "max_users": 50,
  "max_api_keys": 200,
  "max_backends": 20
}
```

---

## Backends

### `GET /backends`
Returns live health + active connection counts enriched from `HealthChecker` + `LoadBalancer`.

### `POST /backends` *(quota-enforced)*
**Request**
```json
{
  "name": "openai-prod",
  "provider_type": "open_ai",
  "endpoint": "https://api.openai.com/v1",
  "credentials": "sk-...",          // encrypted at rest when encryption_key is set
  "priority": 0,
  "weight": 1,
  "timeout_ms": 30000,
  "max_retries": 3
}
```

Provider types: `open_ai`, `anthropic`, `google_vertex`, `aws_bedrock`, `ollama`, `vllm`, `open_ai_compatible`, `qwen`, `xai`, `zai`, `rest`, `graphql`, `grpc`, `generic`.

### `GET|PUT|DELETE /backends/:id`

---

## Routes

Routing rules matched by path prefix. The fallback proxy handler consults these for every non-`/api/v1/*` request.

### `GET /routes` / `POST /routes` / `DELETE /routes/:id`
**POST body**:
```json
{
  "name": "public-api",
  "protocol": "rest",
  "path_pattern": "/public",
  "backend_id": "<uuid>",
  "strip_prefix": true,
  "rewrite_rules": { "/old/(.*)": "/new/$1" }
}
```
Protocols: `rest`, `graphql`, `grpc`, `generic`.

---

## Settings

### `GET /settings` *(TenantAdmin+)*
Returns `{ "settings": { "theme": "dark", "custom_domain": "api.acme.com", ... } }`.

### `PUT /settings` *(TenantAdmin+)*
**Request**: `{ "settings": { "key1": "value1", "key2": "value2" } }`
Upserts keys. Returns the full updated map.

### `DELETE /settings/:key`

---

## LLM (OpenAI-compatible)

All LLM endpoints accept the OpenAI request format. Non-OpenAI providers are translated bidirectionally via `ProviderAdapter`.

### `POST /v1/chat/completions`
Standard OpenAI chat request.

**Special behaviors:**
- Trace context (W3C `traceparent`) injected into upstream requests
- Input tokens counted via `tiktoken` before forwarding (for cost estimation)
- Output tokens counted from response (real cost)
- Usage row written to `usage_records` (fire-and-forget)
- Semantic cache check when `temperature ≤ 0.5` and `stream=false`
- PII detection (when enabled per-tenant)
- If `model: "auto"` — intelligent routing selects tier by prompt complexity
- Streaming via SSE pass-through when `stream=true`

**Headers returned:**
- `x-request-id` — propagated/generated
- `x-gateway-backend` — which backend handled the request
- `x-gateway-cached` — `true` if served from semantic cache

### `POST /v1/completions`
Legacy completions (translated to chat).

### `POST /v1/embeddings`
Embeddings passthrough with cost accounting.

### `GET /v1/models`
Lists registered models in OpenAI format.

### `POST /v1/images/generations` / `/v1/images/edits`
Image generation passthrough.

### `POST /v1/audio/transcriptions` / `/v1/audio/speech`
Audio endpoints passthrough.

---

## Guardrails

### `GET /guardrails` / `GET /guardrails/:id`
List or fetch a guardrail definition.

### `POST /guardrails/test`
**Request**: `{ "pipeline_id": "...", "input": "test content" }`
Dry-run a guardrail pipeline against input without forwarding to any LLM.

**Response 200**:
```json
{
  "outcome": "allow" | "redact" | "block",
  "results": [
    { "guardrail": "pii_detection", "matched": true, "redacted": "..." },
    { "guardrail": "length_limit", "matched": false }
  ]
}
```

---

## Prompts (versioning)

### `GET /prompts`
List prompt names.

### `GET /prompts/:name/versions`
List all versions of a named prompt.

### `GET /prompts/:name/versions/:version`
Fetch a specific version.

### `POST /prompts/:name/deploy`
**Request**: `{ "version": "v3", "percentage": 100 }`
Deploy a version to production. Supports gradual rollout via `percentage`.

### `GET /prompts/:name/deployments`
Current deployment state (A/B split, etc.).

---

## MCP (Model Context Protocol)

### `POST /mcp`
Dual-role MCP endpoint. Accepts JSON-RPC 2.0 with MCP methods:
- `initialize` — handshake, returns server capabilities
- `tools/list`, `tools/call` — aggregated across all registered upstream servers
- `resources/list`, `resources/read` — URI-namespaced
- `prompts/list`, `prompts/get`

Tools are namespaced: `{backend_name}__{tool_name}` to prevent collisions.
Resources: `mcp://{backend_name}/{original_uri}`.

### `GET /mcp/servers` / `POST /mcp/servers` / `DELETE /mcp/servers/:id`
Manage upstream MCP servers. **POST body**:
```json
{
  "name": "github",
  "endpoint": "https://mcp.github.example.com",
  "auth_token": "ghp_..."
}
```

### `POST /mcp/servers/:id/refresh`
Re-discover tools/resources from an upstream server.

### `GET /mcp/tools`
List all aggregated tools across all registered servers.

---

## Webhooks + DLQ

### `GET /webhooks` / `POST /webhooks` / `DELETE /webhooks/:id`
**POST body**: `{ "url": "https://hooks.slack.com/...", "events": ["UserLogin", "*"] }`

The `secret` is auto-generated (format `whsec_...`) and **shown only once** in the create response. Use it to verify HMAC signatures on received events:

```
X-Sentinel-Signature: sha256=<hex HMAC of request body>
```

### `POST /webhooks/:id/test`
Dispatch a synthetic test event to the webhook.

### `GET /webhooks/failures` *(DLQ)*
List recent delivery failures (up to 100).

### `POST /webhooks/failures/:id/retry`
Force-requeue a failed event for immediate retry.

---

## Feedback

*Requires Professional plan or higher. Returns `402 feature_gated` otherwise.*

### `POST /feedback`
Submit end-user feedback on an LLM response.

**Request**
```json
{
  "llm_log_id": "uuid",          // OR "request_id" — one required
  "request_id": "user-supplied-id",
  "rating": 1,                    // -1 | 0 | 1
  "comment": "Helpful answer",
  "metadata": { "source": "web" }
}
```
**Response 201** — `{ "id": "uuid" }`

### `GET /feedback`
List recent feedback (tenant-scoped). Query: `limit` (1–500, default 50).

### `GET /feedback/stats`
Aggregate counts. Query: `days` (1–365, default 30).
```json
{
  "total": 42, "positive": 30, "negative": 12,
  "positive_ratio": 0.71, "window_days": 30
}
```

---

## SSO / OAuth2

*Requires Enterprise plan. Returns `402 feature_gated` otherwise.*

Supports 5 providers: Keycloak, Okta, Google, GitHub, Microsoft Entra.
All OIDC providers use PKCE S256 + CSRF state token. GitHub uses its
non-OIDC OAuth flow (no PKCE).

### Public endpoints (unauthenticated)

### `GET /auth/sso/:slug/authorize`
Redirects the user to the provider's authorize URL. Stores state + PKCE verifier
server-side (single-use).

**Query:** `tenant=<tenant_slug>` *(required)*, `redirect_after=<url>`

### `GET /auth/sso/:slug/callback`
Provider callback. Exchanges code → token → userinfo, auto-provisions user
(when `auto_provision=true`), issues JWT access + refresh tokens.

**Response 200**
```json
{
  "access_token": "eyJ...", "refresh_token": "eyJ...",
  "token_type": "Bearer", "expires_in": 900,
  "user": { "id": "...", "email": "...", "tenant_id": "...", "role": "user" },
  "redirect_after": "/dashboard"
}
```

### Admin endpoints *(TenantAdmin+)*

### `GET /sso/providers`
List SSO providers for the current tenant.

### `POST /sso/providers`
```json
{
  "kind": "keycloak",              // keycloak | okta | google | github | microsoft | oidc_generic
  "display_name": "Acme Keycloak",
  "slug": "acme-keycloak",
  "client_id": "...",
  "client_secret": "...",
  "issuer_url": "https://kc.acme.com/realms/main",  // required for Keycloak/Okta/generic
  "scopes": "openid profile email",
  "default_role": "user",          // user | tenant_admin | read_only
  "auto_provision": true
}
```

### `DELETE /sso/providers/:id`
Soft-deletes (sets `is_active=false`). Existing linked identities remain.

---

## Organizations

*Requires Enterprise plan. SuperAdmin-only. Parent grouping for multiple tenants.*

### `GET /organizations`
List all active organizations.

### `POST /organizations`
```json
{ "slug": "acme", "name": "Acme Corp", "plan": "enterprise", "metadata": {} }
```

### `GET /organizations/:id`
### `DELETE /organizations/:id` — soft-delete
### `GET /organizations/:id/tenants` — UUIDs of tenants under this org
### `POST /organizations/:id/tenants/:tenant_id` — assign tenant to org

---

## Audit Logs

### `GET /audit-logs`

**Query params (cursor pagination):**
- `limit` — default 50, max 200
- `cursor_ts` — RFC3339 timestamp; returns rows strictly before
- `cursor_id` — UUID, required with `cursor_ts` for stable tie-breaking
- `user_id`, `action`, `resource_type` — optional filters

**Response**
```json
{
  "audit_logs": [...],
  "limit": 50,
  "has_more": true,
  "next_cursor": { "ts": "2026-04-17T10:00:00Z", "id": "..." }
}
```

Pass `cursor_ts`/`cursor_id` from `next_cursor` to fetch the next page.

---

## Usage

### `GET /usage`
30-day summary for the caller's tenant.
```json
{
  "period": "30d",
  "total_requests": 12345,
  "total_tokens_input": 1000000,
  "total_tokens_output": 500000,
  "total_tokens": 1500000,
  "total_cost_usd": 42.50
}
```

---

## License

### `GET /license/status`
Current activation state: `unlicensed`, `offline_valid`, `online_valid`, `grace_period`, or `invalid`.
Includes current `features`, `deployment_mode`, `instance_id`.

### `GET /license/features`
Full feature matrix for the current plan. Frontend uses this to drive upsell UI + conditional nav.

**Response 200**
```json
{
  "plan": "professional",
  "features": {
    "plan": "professional",
    "max_requests_per_month": 100000,
    "logs_enabled": true,
    "feedback_enabled": true,
    "semantic_cache_enabled": true,
    "sso_enabled": false,
    "audit_logs_enabled": false,
    "org_management_enabled": false,
    ...
  }
}
```

### `POST /license/activate`
Trigger manual license (re)activation.

### Plan tiers

| Tier | Monthly requests | Retention | Notable features |
|------|------------------|-----------|------------------|
| **Community** (OSS) | Unlimited (self-host) | — | Universal API, fallbacks, loadbalancing, conditional routing, retries, timeouts |
| **Professional** | 100K | 30 days | + Observability (logs/traces/feedback/alerts), caching (simple + semantic), prompts (unlimited), guardrails (+PII), RBAC, teams |
| **Enterprise** | Custom | Custom | + FinOps dashboard, SSO, audit logs, SCIM, JWT auth, BYOK, datalake exports, org management, SOC2/GDPR/BAA, VPC |

### Feature-gated errors

When a handler refuses due to plan:
```json
HTTP 402 Payment Required
{
  "error": {
    "code": "feature_gated",
    "message": "Feature 'sso' requires plan 'enterprise' or higher. Current plan: 'professional'.",
    "feature": "sso",
    "required_plan": "enterprise",
    "current_plan": "professional"
  }
}
```

---

## Platform Sync

Available only in platform-connected deployments (see [docs/architecture.md](architecture.md#deployment-modes)).

### `GET /sync/status`
### `POST /sync/register`
**Request**: `{ "platform_url": "https://...", "admin_email": "...", "instance_name": "..." }`
### `POST /sync/push` / `POST /sync/pull` / `POST /sync/unlink`

---

## Admin *(SuperAdmin)*

### `GET /admin/slow-queries`
Top queries from `pg_stat_statements`. Requires `database.enable_query_stats=true`.

**Query params:** `limit` (1–100, default 20), `min_ms` (default 100.0)
Returns `501 Not Implemented` if the extension is not loaded.

### `POST /admin/slow-queries/reset`
Reset `pg_stat_statements` counters.

### `GET /tenants/:id/license` *(SuperAdmin)*
View the current license status and entitlements for a specific tenant.

**Response 200**
```json
{
  "id": "uuid",
  "tenant_id": "uuid",
  "license_key": "...",
  "status": "active",
  "plan": "professional",
  "entitlements": { ... },
  "expires_at": "2026-12-31T23:59:59Z"
}
```

### `POST /tenants/:id/license` *(SuperAdmin)*
Assign or update a license for a specific tenant.

**Request**
```json
{
  "license_key": "...",
  "plan": "professional",
  "license_type": "online",
  "expires_at": "2026-12-31T23:59:59Z"
}
```
Valid plans: `community`, `professional`, `enterprise`.

---

## Health + Metrics

**Not versioned** — no `/api/v1/` prefix.

### `GET /healthz`
Liveness probe. Returns `{ "status": "ok", "version": "..." }` when process is running.

### `GET /readyz`
Readiness probe — dependency checks. Returns 503 if any dependency is down:
```json
{
  "status": "ready",
  "checks": {
    "database": { "status": "ok", "latency_ms": 5 },
    "rate_limiter": { "status": "ok", "latency_ms": 1 },
    "activation": {...},
    "backends": { "total": 3, "healthy": 3 }
  }
}
```

### `GET /metrics`
Prometheus text format. See [docs/operations.md](operations.md#metrics) for all metrics.

---

## Error Format

All error responses:
```json
{ "error": "Human-readable message" }
```

Common status codes:
| Code | Meaning |
|------|---------|
| 400 | Invalid input (validation error — fields listed in message) |
| 401 | Missing/invalid auth |
| 402 | Budget exceeded |
| 403 | Insufficient permissions / feature not licensed |
| 404 | Resource not found |
| 413 | Request body too large |
| 423 | Account locked |
| 426 | TLS required (when `require_tls=true`) |
| 429 | Rate limited — check `Retry-After`, `X-RateLimit-*` headers |
| 501 | Feature not implemented / extension not loaded |
| 502 | Upstream backend error |
| 503 | Service unavailable (dependency down) |
