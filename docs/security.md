# Security

This document describes Sentinel Gateway's threat model, implemented security controls, and a production hardening checklist.

## Threat model

Sentinel Gateway sits in the critical path between:

- **External clients** (potentially hostile) — AI agents, user apps, SDKs
- **Internal backends** — LLM providers (OpenAI, Anthropic, vLLM) + REST/GraphQL/gRPC services
- **Operational data** — tenants, users, API keys, audit logs, usage records

Primary attacks we defend against:

| Threat | Mitigation |
|---|---|
| Credential theft (JWT/API key leakage) | Short-lived JWTs (15 min), SHA-256 API key hashing, JTI blacklist for revocation, rotation-ready |
| Password brute force | Argon2id hashing, per-IP login rate limit (10/min), account lockout after 5 failures |
| Jailbreak / prompt injection | Regex + semantic guardrails, prompt template enforcement, audit log |
| PII exfiltration | Field encryption (ChaCha20-Poly1305), PII detection/redaction, data-policy filtering by backend |
| Cost exhaustion attacks | RPM + TPM rate limits, CEL cost expressions, hard/soft budget enforcement, circuit breakers |
| Cross-tenant data access | Every query filters by tenant_id, enforced at repository layer |
| Man-in-the-middle | TLS required in platform mode, mTLS-ready, HSTS, CSP, secure cookie flags |
| Audit tampering | Audit log is append-only, no UPDATE/DELETE handler, HMAC-signed webhook delivery |
| Supply chain | Dependency pinning via Cargo.lock, no curl-to-bash, minimal transitive deps |
| Container escape | Non-root user, read-only rootfs in Helm, no privileged mode, NetworkPolicy |

---

## Implemented controls

### Transport

- **TLS termination** at Nginx/Ingress (production) or direct (dev).
- **HSTS** — `max-age=31536000; includeSubDomains` on all responses.
- **CSP** — restrictive default, allows self + data URIs for images. Configured in `frontend/nginx.conf`.
- **X-Frame-Options: DENY, X-Content-Type-Options: nosniff, Referrer-Policy, Permissions-Policy** — all set.
- **`GATEWAY__SERVER__REQUIRE_TLS=true`** — reject non-HTTPS requests in production (middleware returns 426).

### Authentication

- **JWT RS256** — asymmetric signing; workers verify with public key, only the auth service has the private key. Access TTL 15 min, refresh TTL 7 days. Custom `jti` claim for blacklist/revocation.
- **API keys** — 32 random bytes, `sg_` prefix, SHA-256 hashed at rest. **Never stored in plaintext.** Shown once at creation.
- **Password hashing** — Argon2id with per-password salt, memory-hard parameters (resistant to GPU/ASIC attacks).
- **MFA** — schema + `mfa_secret` field exist; TOTP enforcement is future work.
- **SSO/OIDC** — scaffolded in `gateway-auth/src/sso.rs`. Extend for your IdP.
- **CSRF** — double-submit cookie pattern middleware available (`gateway-auth/src/csrf.rs`); wire into web routes as needed.

### Authorization

- **Four roles** — `SuperAdmin`, `TenantAdmin`, `User`, `ReadOnly`. Enforced via middleware `role_gate()`.
- **Route-level** — `/tenants/*` requires `SuperAdmin`; `/settings/*` requires `TenantAdmin`.
- **Scope-based API keys** — each key has a `scopes` array; handlers check before acting.
- **Tenant isolation** — every DB query filters by `tenant_id`. Tenant resolution uses (in order): `X-Tenant-ID` header → JWT `tid` claim → API key lookup → subdomain parse → default tenant (SaaS mode only).

### Rate limiting & abuse prevention

- **Per-IP rate limit** on `/auth/login` — 10 req/min. Applies **before** credential validation.
- **Account lockout** — 5 failed passwords → 15-min lockout. Tracked per-user, not per-IP (so an attacker can't use lockout as a targeted DoS).
- **Global rate limiter** — configurable RPM per tenant/user/key/IP/model. Redis-backed in HA deployments.
- **Token-based (TPM) + CEL cost expressions** — see [rate-limiting.md](features/rate-limiting.md).
- **Budgets** — daily/monthly hard and soft limits with alert webhooks.

### Data protection

- **Encryption at rest** — ChaCha20-Poly1305 AEAD via `gateway-core::FieldEncryptor`. Used for backend credentials, optionally for settings values (per-row flag). Key configured via `GATEWAY__SERVER__ENCRYPTION_KEY` (32-byte hex). Rotate via offline key rotation script (future work).
- **PII detection** — built into the `gateway-llm::pii` module and as a first-class guardrail kind. Modes: detect, redact, block.
- **Data policy filter** — each backend has a `data_policy` (`standard` / `no_retention` / `no_training` / `strict`). Tenants can require minimum policy via `X-Min-Data-Policy` header — see [features/guardrails.md](features/guardrails.md).
- **Secret redaction in logs** — the telemetry middleware strips `Authorization`, `X-API-Key`, `Cookie` headers before logging.
- **No secrets in error messages** — frontend error interceptor in `frontend/src/lib/api.ts` sanitizes status-code messages before showing to users.

### SQL injection / input validation

- **SQLx compile-time checked queries** where used. All user-supplied values bound via `$1`, `$2` placeholders — never string interpolation.
- **Validator crate** — every request struct uses `#[validate]` derives with email, URL, length, range constraints. Validation runs before any business logic.
- **GraphQL query depth limiting** — `GATEWAY__PROXY__GRAPHQL_MAX_DEPTH` (default 10) rejects deep nested queries before forwarding.
- **Max body size** — `GATEWAY__PROXY__MAX_BODY_SIZE` (default 10MB) caps request bodies at the Axum layer.

### Audit

- **Immutable audit log** — append-only `audit_logs` table. Every admin action (user invite, key revoke, tenant create, etc.) produces an event.
- **Async buffered writer** — 100 events or 5s flush, so request latency isn't affected.
- **Webhooks for audit events** — HMAC-signed with per-endpoint secret, retry with exponential backoff, dead-letter queue (`webhook_failures`) with manual retry UI.
- **Audit IP extraction** — honors `X-Forwarded-For` → `X-Real-IP` → direct connection, so behind a load balancer you still see the real client IP.

### Operational

- **Non-root container user** — `sentinel:sentinel` (uid 65534).
- **Read-only rootfs + emptyDir for `/tmp`** — enforced in Helm chart.
- **Graceful shutdown** — 30s `terminationGracePeriodSeconds` + preStop hook, in-flight requests complete.
- **Secret volumes** — Helm mounts RSA keys from K8s Secret, never baked into images.
- **NetworkPolicy** — optional per-tenant network isolation in Helm chart.

---

## Production hardening checklist

Before going to prod, confirm:

### Secrets & keys

- [ ] **`GATEWAY__SERVER__ENCRYPTION_KEY`** set to a real 32-byte hex key (not the `.env.example` default)
- [ ] **RSA keys** generated, private key has 0600 permissions, **not** in the container image — mount via Secret
- [ ] **Database password** is strong, unique, and NOT `sentinel_password`
- [ ] **Redis password** set (`REDIS_PASSWORD` in prod compose)
- [ ] **License key** (if platform mode) stored in a Secret, not a ConfigMap

### Network

- [ ] `GATEWAY__SERVER__REQUIRE_TLS=true`
- [ ] TLS certificate via cert-manager (Helm) or terminated at Ingress
- [ ] `GATEWAY__SERVER__CORS_ALLOW_ALL=false`, `CORS_ORIGINS` configured
- [ ] NetworkPolicy enabled in Helm (`networkPolicy.enabled=true`)
- [ ] Ingress rate-limiting enabled (nginx `limit_req` or Cloudflare)

### Auth

- [ ] First admin created via `gateway-server create-admin`, initial password changed
- [ ] Lockout + login rate limit tested (simulate 10 bad logins, confirm 429)
- [ ] Refresh tokens short enough for your risk tolerance (default 7d; consider 1d for high-security)
- [ ] API key scopes narrowed — don't issue `admin`-scoped keys to apps

### Observability

- [ ] OTLP endpoint configured
- [ ] Prometheus scraping `/metrics`
- [ ] Alerts wired up for:
  - [ ] HTTP 5xx rate > 1% for 5 min
  - [ ] p99 latency > 1s for 10 min
  - [ ] DB connection saturation > 80%
  - [ ] Webhook DLQ growing (backlog > 100)
  - [ ] Audit log write failures
  - [ ] Rate-limit violations spike (possible attack)
- [ ] Runbooks linked from each alert

### Data

- [ ] PostgreSQL backups automated, restore tested
- [ ] Audit log retention configured (default 365d for Enterprise, 30d for Community)
- [ ] Usage records pruning if long-term storage isn't needed
- [ ] Tenant delete cascade tested — data actually goes away

### Supply chain

- [ ] `Cargo.lock` committed
- [ ] Dependabot / Renovate for Rust + npm dependencies
- [ ] `cargo audit` in CI
- [ ] No unpinned container tags — always pin to SHA256 in prod
- [ ] Image signing (cosign) if using a registry that supports it

### Compliance prep (if applicable)

- [ ] SOC2 — audit log + access controls + encryption at rest ✅ all in place; you'll need quarterly access reviews + incident response process
- [ ] GDPR — user/tenant delete is cascading + pseudonymous; implement your own data export endpoint
- [ ] HIPAA — requires BAA with your LLM providers; use `data_policy = strict` backends only for PHI; enable audit log for all reads (future work)
- [ ] PCI — don't handle card data in prompts; guardrails can redact on the way in but relying on that for compliance is fragile

---

## Known limitations / future work

- **SSO is scaffolded, not wired.** `gateway-auth/src/sso.rs` has the provider-agnostic trait. Implement for your IdP (Okta, Auth0, Azure AD).
- **MFA is schema-only.** `users.mfa_secret` exists but enforcement at login time is TODO.
- **CSRF middleware not wired by default.** It's header-auth-aware (skips when `Authorization` or `X-API-Key` are present). Enable for cookie-based web sessions when you add them.
- **Key rotation for field encryption** is manual. Future work: graceful rotation with two-key support.
- **Read-audit events** — we log admin *writes* but not reads. If you need `who viewed this audit log` trails, extend `AuditService::log()` calls to read handlers.
- **No bundled WAF** — rely on upstream (Cloudflare, AWS WAF). We don't try to be an L7 firewall.
- **mTLS to backends** — TLS verification is on (rustls), but mTLS client certs to upstream providers isn't yet a first-class feature. Workaround: sidecar proxy that adds the cert.

---

## Reporting vulnerabilities

**Do not** open public issues for security bugs. Email **security@sentinel-gateway.example** with:

1. Affected version
2. Steps to reproduce
3. Impact assessment
4. Proof of concept (if available)

We aim to respond within 48 hours. Coordinated disclosure window is 90 days unless otherwise agreed.

---

## See also

- [Configuration](configuration.md) — every env var that affects security posture
- [Architecture](architecture.md#key-architectural-decisions) — why we chose the primitives we chose
- [Operations](operations.md) — day-2 operations including key rotation and incident response
