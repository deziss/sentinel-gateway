# Rate Limiting

Sentinel Gateway offers three layers of rate limiting:

1. **Standard RPM** — requests per minute per key (tenant / user / API key / IP).
2. **Token-based limits** — tokens per minute (TPM), accounting for the fact that LLM requests vary 100x in cost.
3. **CEL cost expressions** — programmable cost functions like `input * 1 + output * 3` for price-accurate rate limiting.

## Why three layers

- RPM is simple and protects against bot floods.
- TPM accounts for the difference between a "hi" and a 10K-token code review.
- CEL expressions let you bill output tokens 3x input, discount cached tokens 10x, and penalize reasoning tokens — matching your actual provider invoice.

## Scopes

All layers share the same scopes:

| Scope | Use case |
|---|---|
| Per **API key** | SaaS customer-facing keys — each customer has their own quota |
| Per **user** | Human operators in a shared tenant |
| Per **tenant** | Tenant-wide ceiling across all users/keys |
| Per **IP** | Anonymous / pre-auth routes (e.g. login — see `handlers/auth.rs` for the 10 req/min per IP login limit) |
| Per **model** | Reserve capacity on a scarce/expensive model |

---

## Standard RPM (default)

Implemented in `gateway_policy::RateLimiter`. Two algorithms, both selectable at startup:

- **Token bucket** — smooth, bursts allowed up to capacity.
- **Sliding window** — precise per-window counts, slightly higher overhead.

Backends:
- **In-memory** (default) — DashMap per key, fine for single replica.
- **Redis** — required when running multiple replicas. Uses a Lua script for atomic check-and-decrement.

Enabled by setting API key fields at creation time:

```bash
curl -X POST /api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "production",
    "scopes": ["proxy"],
    "rate_limit_rpm": 1000,
    "budget_monthly": 500.00
  }'
```

---

## CEL cost expressions (P2 moat feature)

The real money saver. Instead of counting requests, charge by **computed cost units**.

### Example expressions

```text
# Uniform — treat all tokens equally
input + output

# OpenAI-style — output is ~3x the cost of input
input * 1 + output * 3

# Discount cached tokens (OpenAI / Anthropic prompt caching) — they cost 10% of a fresh token
(input - cached) * 1 + cached * 0.1 + output * 3

# Penalize reasoning tokens (o1, Claude 3.7 thinking) — they're expensive
input + output * 3 + reasoning * 10

# Model-aware — premium models cost more
model == "gpt-4o" ? input * 2.5 + output * 10 : input * 0.5 + output * 1.5

# Tenant-specific discount
tenant == "enterprise-acme" ? (input + output) * 0.5 : input + output * 3
```

### Available variables

| Variable | Alias | Type | Source |
|---|---|---|---|
| `input` | `prompt_tokens` | int | Token count of the request |
| `output` | `completion_tokens` | int | Token count of the response |
| `cached` | `cached_tokens` | int | Tokens served from provider cache |
| `cache_creation` | `cache_creation_tokens` | int | Tokens that populated the cache |
| `reasoning` | `reasoning_tokens` | int | Hidden reasoning tokens (o1, Claude 3.7) |
| `total` | `total_tokens` | int | `input + output` |
| `model` | — | string | The model name (e.g. `"gpt-4o"`) |
| `tenant` | — | string | Tenant ID as string |

### How it works

1. On server boot, CEL expressions are **parsed once** via `cel-interpreter::Program::compile`. Bad syntax fails fast at load time — no surprises in prod.
2. Parsed programs are cached in a `CelRateLimitRegistry` keyed by name.
3. On LLM request completion (after we know final token counts), the expression is **evaluated once** (~1µs). The resulting integer becomes the cost debit.
4. The cost is charged to the rate limiter via `RateLimiter::consume(key, limit_per_minute, cost)`.

Blocking/allowing decisions happen on the **next** request — the current request always completes so you can see final token usage in logs.

### Programmatic registration

```rust
use gateway_policy::{CelRateLimitRegistry, TokenVars};

let registry = CelRateLimitRegistry::new();
registry.set(
    "premium-models",
    "model == \"gpt-4o\" ? input * 5 + output * 15 : input + output",
    10_000,        // 10,000 cost units per minute
    "per tenant",
)?;

// At request end:
let cel = registry.get("premium-models").unwrap();
let vars = TokenVars::new(prompt_tokens, completion_tokens, "gpt-4o")
    .with_cached(cached_tokens, cache_creation_tokens)
    .with_reasoning(reasoning_tokens)
    .with_tenant(tenant_id.to_string());
cel.consume(&rate_limiter, &RateLimitKey::Tenant(tenant_id), &vars).await?;
```

Future: UI for configuring CEL expressions per tenant, config-file-driven registration.

---

## What happens on rate-limit violation

- HTTP **429 Too Many Requests** with headers:
  - `Retry-After: 60` (seconds until the next bucket refill)
  - `X-Ratelimit-Limit: <limit>`
  - `X-Ratelimit-Remaining: 0`
- Body: `{"error": "Too many requests. Please try again later."}`
- Metric `gateway_rate_limited_total{tenant,key_type}` increments.
- Audit event `RateLimitExceeded` recorded.

---

## Authentication endpoint protection

The `/api/v1/auth/login` endpoint has a dedicated per-IP limit of **10 req/min** (see `handlers/auth.rs`). This runs before credential validation — even enumerating emails is rate-limited. On violation you get 429 with `Retry-After: 60`.

Additionally:
- **Account lockout** — after `max_failed_logins` (default 5) bad passwords, the user is locked for `lockout_duration_minutes` (default 15). Configurable via `GATEWAY__AUTH__*`.
- **Failed login attempts** are tracked in `users.failed_login_attempts` and counted per user (so an attacker can't use the lockout to DoS a specific account — they're locked out long before they get many attempts).

---

## Multi-replica deployments

When you run `GATEWAY__REPLICAS > 1`, you **must** configure Redis. Without it, each replica has its own in-memory buckets and the effective rate limit becomes `limit × replicas`. The gateway refuses to start in multi-replica mode without Redis — a fatal error.

```bash
GATEWAY__REPLICAS=3
GATEWAY__REDIS__URL=redis://redis:6379
```

The Redis variant uses a single Lua script for atomic check-and-decrement. Network round-trip adds ~1ms p99.

---

## Budgets (complementary)

Rate limiting answers "how often?" — budgets answer "how much?". Set in `api_keys`:

- `budget_daily` — e.g. `$10/day`
- `budget_monthly` — e.g. `$500/month`
- Hard limits block further requests on exceed
- Cost metric already tracked per usage record; budget enforcer sums live

See `gateway_policy::BudgetEnforcer` for internals.

---

## Patterns

### Cost-accurate premium tier

Charge premium tenants 1x per token, free tier 3x (effectively rate-limiting free tier harder):

```text
tenant_plan == "premium" ? input + output : (input + output) * 3
```

### Fair model allocation

Reserve 10K tokens/min on `gpt-4o` for each user:

```rust
let cel = CostExpression::parse("input + output").unwrap();
// Per-user-per-model composite key
let key = RateLimitKey::Composite(user_id.to_string(), "gpt-4o".to_string());
cel.consume(&limiter, &key, &vars).await?;
```

### Prompt-cache incentives

Reward users who leverage prompt caching:

```text
(input - cached) * 1 + cached * 0 + output * 3
```

Cached tokens effectively cost nothing — encourages users to structure requests for cacheability.

---

## Caveats

- **Cost debits happen post-request** — a single very expensive request isn't blocked, but it counts against the next window. Combine with RPM for pre-request defense.
- **No CEL UI yet** — today you configure via `CelRateLimitRegistry` in code or a future config file. UI is on the roadmap.
- **Redis dep for HA** — in-memory is single-replica only.

---

## See also

- [Semantic Policies](semantic-policies.md) — block based on content, not just frequency
- [Observability](observability.md) — see rate-limit + cost metrics in Prometheus/OTel
