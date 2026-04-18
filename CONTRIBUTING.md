# Contributing to Sentinel Gateway

Thanks for your interest! This guide covers local setup, the dev loop, and PR expectations.

## Prerequisites

- **Rust** 1.85+ (`rustup update stable`)
- **Docker** + **Docker Compose**
- **PostgreSQL** 16 client tools (`psql`, `pg_dump`) for manual inspection

## Quick Start (5 minutes)

```bash
git clone <repo>
cd sentinel-gateway

# Start Postgres + Redis + OTEL Collector (dev deps only)
docker compose up -d db redis otel-collector

# Build + test
cd backend
cargo test --workspace

# Run the server locally against the Docker DB
export DATABASE_URL=postgres://sentinel:sentinel_password@localhost:5438/sentinel_gateway
cargo run --bin gateway-server -- serve
```

Gateway is now live on `http://localhost:8080`. Check `/healthz` to confirm.

## Project Layout

See [docs/architecture.md](docs/architecture.md) for full layout. TL;DR:

```
backend/
  crates/           ← 12 domain crates
    gateway-db/     ← models + repositories + migrations
    gateway-auth/   ← JWT, API keys, RBAC
    gateway-core/   ← proxy, LB, CB, WebSocket, GraphQL, gRPC
    gateway-llm/    ← providers, adapters, cost, cache, PII
    gateway-policy/ ← rate limit, budget, guardrails
    gateway-server/ ← main binary, routes, handlers, CLI
    gateway-tests/  ← integration tests (lib + 5 test files)
    ...
  migrations/       ← SQL migrations (1–13+)
deploy/
  helm/             ← Kubernetes Helm chart
  prometheus/       ← SLO alerts + scrape config
  grafana/          ← Dashboard JSON
docs/               ← Markdown docs (architecture, api, ops, ...)
```

## Dev Loop

### Running tests

```bash
# Everything (fast — 104 unit/integration tests, no DB required)
cargo test --workspace

# A specific crate
cargo test -p gateway-auth

# A specific test
cargo test -p gateway-tests --test llm_flow -- semantic_cache_hits_on_identical_request

# With DB-backed tests (requires TEST_DATABASE_URL)
TEST_DATABASE_URL=postgres://... cargo test --workspace
```

### Lints

```bash
cargo fmt --all          # format (required)
cargo clippy --all-targets --all-features -- -D warnings
```

CI rejects PRs that fail `fmt` or `clippy` (see `.github/workflows/ci.yml`).

### Database migrations

Migrations live in `backend/migrations/NNN_description.sql`. Run them with:

```bash
cargo run --bin gateway-server -- migrate
```

**Rules:**
- New migrations get the next sequential number. **Never rewrite existing migrations** — add a new one.
- Use `CREATE TABLE IF NOT EXISTS` for safe re-runs.
- Indexes on tables with >1M rows must use `CREATE INDEX CONCURRENTLY`.
- New NOT NULL columns must have a DEFAULT or run as expand-migrate-contract.
- If migration takes >30 s on prod-size data, split into background job.
- Test forward AND backward (rollback) on a prod-sized copy before merging.

### Adding an endpoint

1. **Handler** → `crates/gateway-server/src/handlers/<module>.rs`
2. **Route** → register in `crates/gateway-server/src/routes/mod.rs` under the right role gate
3. **Validation** → use the `validator` crate on request structs (see `handlers/auth.rs`)
4. **Audit** → emit `AuditEvent` for any state-changing action
5. **Test** → add to `crates/gateway-tests/tests/`
6. **Docs** → update `docs/api-reference.md`

### Adding an LLM provider

1. Add a variant to `gateway_llm::provider::ProviderType` + `gateway_db::models::BackendProviderType` (both must match)
2. Add a migration `NNN_add_provider_foo.sql`:
   ```sql
   ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'foo';
   ```
3. Add `chat_url()`, `embeddings_url()`, `models_url()`, `auth_header()` cases in `provider.rs`
4. Add pricing in `cost.rs`
5. If the provider speaks non-OpenAI format, add `to_foo()` + `foo_response_to_openai()` adapters in `adapter.rs` and wire them into `adapt_request()` / `adapt_response()` dispatch
6. Add a test in `crates/gateway-tests/tests/llm_flow.rs`

## PR Expectations

### Required checks
- ☐ `cargo fmt --all -- --check` passes
- ☐ `cargo clippy --all-targets --all-features -- -D warnings` passes
- ☐ `cargo test --workspace` passes
- ☐ New endpoints have integration tests (valid input, auth, invalid input)
- ☐ New migrations tested forward + backward
- ☐ New env vars documented in `docs/configuration.md` AND `backend/.env.example`
- ☐ New dependencies reviewed for supply-chain risk (prefer widely-used crates)

### PR description template

```markdown
## What
One-paragraph summary.

## Why
The problem this solves.

## How
Key implementation decisions. Mention any non-obvious tradeoffs.

## Breaking changes
☐ None
☐ Yes → list with migration path

## Testing
How to verify locally + what automated tests cover.

## Docs
☐ Updated docs/api-reference.md
☐ Updated docs/configuration.md
☐ Updated docs/operations.md (if ops-visible)
```

### Commit style

Conventional commits preferred:
- `feat(auth): add per-IP login rate limit`
- `fix(proxy): correct circuit breaker state transition`
- `docs(ops): document DLQ retry procedure`
- `refactor(llm): consolidate adapter dispatch`
- `test(policy): cover sliding window edge cases`

## Security

Report security issues privately to **security@example.com** (do NOT open a public issue).

Never commit:
- `.env` files with real secrets
- Private keys (the `keys/` directory is gitignored)
- License keys
- Tokens of any kind

Pre-commit hook suggestion:
```bash
# .git/hooks/pre-commit
#!/bin/sh
if git diff --cached | grep -E "(password|api.?key|secret).*=.*['\"][^'\"]+['\"]"; then
    echo "⚠️  Possible secret in diff — review before committing"
    exit 1
fi
```

## Code Review

- PRs need one approval before merge.
- Maintainers may ask for changes based on the [backend-architect skill rules](https://github.com/anthropic-skills/backend-architect) — e.g., cursor pagination over OFFSET, partial indexes on filtered queries, parameterized queries only.
- If you introduce a new external service dependency, include a health-check + circuit breaker per the skill's reliability rules.

## Licensing

Sentinel Gateway is MIT-licensed. By contributing, you agree your contributions are also MIT-licensed. See [LICENSE](LICENSE).
