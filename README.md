# Sentinel Gateway

> **Universal AI gateway in Rust** — REST, GraphQL, gRPC, and LLM proxying with enterprise governance, offline-first licensing, and a dual-role MCP server.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Protocol](https://img.shields.io/badge/protocols-REST%20%7C%20GraphQL%20%7C%20gRPC%20%7C%20WS%20%7C%20MCP-blue)](docs/features/)

## Value proposition

*"The only AI gateway that proxies every protocol (REST / GraphQL / gRPC / MCP), runs air-gapped, and enforces policies based on what the prompt actually means — without sending your data to a SaaS."*

Three defensible claims:

1. **Protocol universality** — no other gateway handles all four protocols natively.
2. **Air-gapped operation** — offline license validation, no phone-home, self-hosted economics.
3. **Semantic policies** — route or block based on prompt meaning, not just keywords.

## What's in the box

| Capability | Details |
|---|---|
| **Universal proxy** | REST, GraphQL (depth limiting, introspection passthrough), gRPC (Tonic), WebSocket, generic HTTP |
| **LLM module** | 17 providers (OpenAI, Anthropic, Google, AWS, Mistral, Cohere, DeepSeek, Groq, Together, Perplexity, Fireworks, xAI, Qwen, Zai, vLLM, Ollama, OpenAI-compatible) |
| **MCP gateway** | Dual-role: serves tools to AI agents, aggregates from upstream MCP servers (namespaced) |
| **Prompt management** | Versioning, label-based deployments (prod/staging/canary), variable rendering |
| **Guardrails** | Pluggable trait-based pipeline with regex, PII, length, JSON schema, semantic checks |
| **Rate limiting** | Per-tenant / per-user / per-key, token-bucket + sliding window, Redis-distributed, **CEL cost expressions** |
| **Semantic policies** | Embedding-based policy engine — block/flag/route based on prompt meaning |
| **Inference-aware routing** | Routes to vLLM/TGI/SGLang backends by queue depth + KV-cache usage |
| **Budget enforcement** | Hard/soft limits per tenant/user/key, daily/weekly/monthly windows |
| **Auth** | JWT RS256, API keys (SHA-256 hashed, `sg_*` prefix), RBAC with 4 roles, SSO/OIDC scaffold, CSRF middleware, per-IP login rate limit |
| **Security** | ChaCha20-Poly1305 field encryption, Argon2id passwords, input validation (validator crate), CORS + HSTS + CSP, secret redaction in logs |
| **Audit & webhooks** | Immutable audit log, HMAC-signed webhook delivery with retry + DLQ |
| **Telemetry** | OpenTelemetry (traces + metrics + logs), Prometheus, W3C TraceContext, **optional export to Langfuse / Helicone** |
| **Multi-tenancy** | Subdomain/header/JWT resolution, per-tenant feature flags, quotas, SaaS toggle |
| **Offline license** | RSA signature verification, no phone-home, community / professional / enterprise plans |

## Repository layout

```text
sentinel-gateway/
├── backend/                        # Rust workspace — 11 crates, 17 migrations
│   ├── crates/
│   │   ├── gateway-core/           # Proxy engine + load balancer + crypto + inference metrics
│   │   ├── gateway-auth/           # JWT + API keys + CSRF + token blacklist
│   │   ├── gateway-tenant/         # Multi-tenancy + SaaS mode
│   │   ├── gateway-license/        # Offline license + feature gates
│   │   ├── gateway-llm/            # 17 providers + token counting + cost + PII + smart routing + semantic cache
│   │   ├── gateway-policy/         # Rate limiting + budgets + IP filter + guardrails + CEL + semantic
│   │   ├── gateway-telemetry/      # OpenTelemetry + Prometheus
│   │   ├── gateway-audit/          # Immutable audit + webhooks + DLQ
│   │   ├── gateway-db/             # SQLx + 12 tables
│   │   ├── gateway-mcp/            # MCP dual-role gateway (protocol + client + server + registry)
│   │   └── gateway-server/         # Axum binary + handlers + routes
│   └── migrations/                 # 001–017 SQL migrations
├── frontend/                       # React 19 + Vite 6 + Tailwind 4 + shadcn/ui + fp-ts + Zustand
│   └── src/pages/                  # Dashboard, Backends, Users, API Keys, Routes, Audit, Settings,
│                                   # LlmPlayground, LlmAnalytics, LlmCatalog, McpServers, Prompts, Guardrails
├── deploy/
│   ├── docker/                     # OTel collector config
│   └── helm/sentinel-gateway/      # Chart.yaml, values.yaml, 13 templates (HA-ready)
├── docker-compose.yml              # Dev stack (db, redis, keygen, migrator, backend, frontend, otel)
├── docker-compose.prod.yml         # Production overlay (PgBouncer, resource limits, required secrets)
└── docs/                           # Full documentation — you are here
    ├── architecture.md             # System design
    ├── api.md                      # REST API reference
    ├── configuration.md            # Env vars + config reference
    ├── security.md                 # Threat model + security practices
    ├── CHANGELOG.md                # Release notes
    └── features/
        ├── prompts.md
        ├── guardrails.md
        ├── mcp.md
        ├── rate-limiting.md
        ├── semantic-policies.md
        ├── inference-routing.md
        └── observability.md
```

## Quick start

### Docker Compose (recommended for local dev)

```bash
git clone <repo-url>
cd sentinel-gateway
docker compose up -d
# → http://localhost:3005 (UI)
# → http://localhost:8080 (API)
# → http://localhost:8080/metrics (Prometheus)
```

The compose file provisions PostgreSQL 16, Redis 7, an OTel collector, generates RSA keys, runs migrations, and starts both backend and frontend. First boot takes ~2 minutes (mostly Rust compile).

### From source

**Prerequisites:** Rust 1.85+, Node.js 20+, PostgreSQL 16+

```bash
# Terminal 1 — infrastructure
docker compose up -d db redis otel-collector keygen migrator

# Terminal 2 — backend
cd backend
cp .env.example .env  # edit as needed
cargo run -p gateway-server -- serve

# Terminal 3 — frontend
cd frontend
npm install
npm run dev
```

### Create the first admin user

```bash
cd backend
cargo run -p gateway-server -- create-admin \
  --email admin@acme.com \
  --password 'strong-password-here' \
  --tenant-slug default
```

### Test an LLM request

```bash
# Get an access token
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"default","email":"admin@acme.com","password":"..."}'

# Send a chat completion (OpenAI-compatible)
curl -X POST http://localhost:8080/v1/chat/completions \
  -H 'Authorization: Bearer <access_token>' \
  -H 'X-Tenant-ID: <tenant_id>' \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role":"user","content":"Say hello"}]
  }'
```

## Configuration

All settings use env vars prefixed `GATEWAY__`, nested with `__`. See [`docs/configuration.md`](docs/configuration.md) for the full reference.

Most-used variables:

| Variable | Description | Default |
|---|---|---|
| `GATEWAY__DATABASE__URL` | PostgreSQL connection string | `postgres://sentinel:sentinel@localhost:5432/sentinel_gateway` |
| `GATEWAY__REDIS__URL` | Redis URL (enables distributed rate limiting) | — |
| `GATEWAY__SERVER__SAAS_MODE` | Single-tenant SaaS mode | `false` |
| `GATEWAY__SERVER__DEPLOYMENT_MODE` | `local` (free) or `platform` (licensed) | `local` |
| `GATEWAY__SERVER__ENCRYPTION_KEY` | 32-byte hex for ChaCha20 field encryption | — |
| `GATEWAY__AUTH__JWT_PRIVATE_KEY_PATH` | RS256 private key PEM | `keys/private.pem` |
| `GATEWAY__AUTH__JWT_PUBLIC_KEY_PATH` | RS256 public key PEM | `keys/public.pem` |
| `GATEWAY__TELEMETRY__OTLP_ENDPOINT` | OTel collector endpoint | — |
| `GATEWAY__OBSERVABILITY__LANGFUSE__*` | Optional Langfuse export | disabled |
| `GATEWAY__OBSERVABILITY__HELICONE__*` | Optional Helicone export | disabled |
| `GATEWAY__LICENSE__LICENSE_KEY` | Enterprise license key | — |

## Deployment

### Kubernetes (Helm)

```bash
helm install sentinel-gateway ./deploy/helm/sentinel-gateway \
  --namespace sentinel --create-namespace \
  --set image.tag=v1.0.0 \
  --set ingress.enabled=true \
  --set ingress.hosts[0].host=gateway.example.com
```

The chart includes HPA (3–20 replicas), PDB (min 2 available), pod anti-affinity, RBAC, NetworkPolicy, cert-manager integration, and subchart dependencies for PostgreSQL and Redis. See [`docs/configuration.md`](docs/configuration.md) for values overrides.

### Production Docker Compose

```bash
cp .env.example .env  # set strong POSTGRES_PASSWORD and REDIS_PASSWORD
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

The production overlay adds PgBouncer, resource limits, log rotation, and requires secret env vars via `:?` — fails to start if any are missing.

## License plans

Enforced by the offline license validator. Without a license, the gateway runs in `local` mode with community limits — no phone-home, works air-gapped.

| Plan | Users | API keys | Backends | RPM | Budget/mo | SSO | gRPC | Multi-tenant | Audit retention |
|---|---|---|---|---|---|---|---|---|---|
| **Community** | unlimited | unlimited | unlimited | unlimited | unlimited | ❌ | ❌ | ❌ | 30d |
| **Professional** | 50 | 200 | 20 | 1,000 | $500 | ✅ | ❌ | ✅ | 90d |
| **Enterprise** | unlimited | unlimited | unlimited | unlimited | unlimited | ✅ | ✅ | ✅ | 365d |

## Documentation

| Document | For |
|---|---|
| [Architecture](docs/architecture.md) | System design, crate map, request lifecycle, scaling thresholds |
| [API Reference](docs/api-reference.md) | 53 endpoints, auth methods, cursor pagination, error format |
| [Configuration](docs/configuration.md) | Every env var / TOML key, staging + production profiles |
| [Deployment](docs/deployment.md) | Docker, Kubernetes/Helm, bare metal, post-deploy verification |
| [Operations](docs/operations.md) | SLOs, alert severity, runbooks, capacity planning, incident response |
| [Contributing](CONTRIBUTING.md) | Dev setup, test loop, PR expectations, adding endpoints/providers |
| [Security](docs/security.md) | Threat model, security practices, compliance notes |
| [CHANGELOG](docs/CHANGELOG.md) | Release history |

**Feature guides:**

| Feature | Guide |
|---|---|
| Prompt management & versioning | [docs/features/prompts.md](docs/features/prompts.md) |
| Guardrails pipeline | [docs/features/guardrails.md](docs/features/guardrails.md) |
| MCP dual-role gateway | [docs/features/mcp.md](docs/features/mcp.md) |
| Rate limiting (CEL cost expressions) | [docs/features/rate-limiting.md](docs/features/rate-limiting.md) |
| Semantic policy engine | [docs/features/semantic-policies.md](docs/features/semantic-policies.md) |
| Inference-aware routing (vLLM/TGI) | [docs/features/inference-routing.md](docs/features/inference-routing.md) |
| Observability (OTel + optional export) | [docs/features/observability.md](docs/features/observability.md) |

## Project status

- Core proxy, auth, tenancy, LLM routing, audit — **production-ready**
- MCP gateway (protocol 2025-06-18), prompt management, guardrails — **production-ready**
- Semantic policy engine — hashing-trick embedder production-ready; HTTP embedder production-ready; ONNX-based embedder is **future work**
- KV-cache-aware routing — works against vLLM, TGI, SGLang metrics; falls back to least-connections when backends don't expose `/metrics`
- Observability export — Langfuse and Helicone supported, opt-in
- Helm chart — **complete, not battle-tested at scale**

## License

MIT © Sentinel Gateway Contributors
