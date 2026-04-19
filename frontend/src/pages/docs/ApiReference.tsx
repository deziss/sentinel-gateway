import { H1, Lead, H2, H3, P, UL, Code, Pre, Callout, Endpoint, Table, THead, TH, TR, TD } from "./_primitives"

export function ApiReference() {
  return (
    <div>
      <H1>API Reference</H1>
      <Lead>
        Complete REST reference. All endpoints return JSON; all mutating endpoints
        require authentication; all error responses use the shape{" "}
        <Code>{"{ error: { type, message } }"}</Code>.
      </Lead>

      <H2 id="auth-scheme">Authentication schemes</H2>
      <P>There are three ways to authenticate:</P>
      <Table>
        <THead>
          <TR>
            <TH>Scheme</TH>
            <TH>Header</TH>
            <TH>Use for</TH>
          </TR>
        </THead>
        <tbody>
          <TR>
            <TD>JWT (session)</TD>
            <TD><Code>Authorization: Bearer &lt;jwt&gt;</Code></TD>
            <TD>Frontend / user-facing admin</TD>
          </TR>
          <TR>
            <TD>API key</TD>
            <TD><Code>Authorization: Bearer sg_...</Code></TD>
            <TD>Server-to-server LLM calls</TD>
          </TR>
          <TR>
            <TD>Tenant header</TD>
            <TD><Code>X-Tenant-Id: &lt;uuid&gt;</Code></TD>
            <TD>Combined with either above to scope cross-tenant admin</TD>
          </TR>
        </tbody>
      </Table>

      <H2 id="errors">Error shape</H2>
      <Pre lang="json">{`{
  "error": {
    "type": "rate_limit_exceeded",
    "message": "RPM exceeded for key; retry after 12s",
    "retry_after": 12
  }
}`}</Pre>

      <H2 id="auth">Auth</H2>

      <H3>Login</H3>
      <Endpoint method="POST" path="/auth/login" />
      <Pre lang="request">{`{ "email": "alice@example.com", "password": "..." }`}</Pre>
      <Pre lang="response 200">{`{
  "access_token": "<jwt, 15min>",
  "refresh_token": "<jwt, 7d>",
  "user": { "id": "...", "email": "...", "role": "admin" },
  "tenant_id": "..."
}`}</Pre>

      <H3>Refresh</H3>
      <Endpoint method="POST" path="/auth/refresh" />
      <Pre lang="request">{`{ "refresh_token": "..." }`}</Pre>

      <H3>Logout</H3>
      <Endpoint method="POST" path="/auth/logout" />

      <H3>Change password</H3>
      <Endpoint method="POST" path="/auth/change-password" />
      <Pre lang="request">{`{ "current_password": "...", "new_password": "..." }`}</Pre>

      <H2 id="llm-proxy">LLM proxy (OpenAI-compatible)</H2>

      <H3>Chat completions</H3>
      <Endpoint method="POST" path="/v1/chat/completions" />
      <P>Forwards to the provider resolved from the model in the body. Accepts all OpenAI fields plus:</P>
      <UL>
        <li><Code>prompt_ref</Code> — reference a managed prompt template</li>
        <li><Code>variables</Code> — values for template placeholders</li>
      </UL>
      <Pre lang="request">{`{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello"}
  ],
  "temperature": 0.7,
  "max_tokens": 1000,
  "stream": false
}`}</Pre>

      <H3>Completions (legacy)</H3>
      <Endpoint method="POST" path="/v1/completions" />

      <H3>Embeddings</H3>
      <Endpoint method="POST" path="/v1/embeddings" />
      <Pre lang="request">{`{ "model": "text-embedding-3-small", "input": "hello world" }`}</Pre>

      <H3>List models</H3>
      <Endpoint method="GET" path="/v1/models" />

      <Callout kind="info">
        These endpoints are drop-in compatible with the OpenAI client SDKs. Point
        <Code>base_url</Code> at your gateway and use an <Code>sg_</Code>-prefixed key.
      </Callout>

      <H2 id="tenants">Tenants</H2>

      <H3>List tenants (global admin)</H3>
      <Endpoint method="GET" path="/api/tenants" />

      <H3>Get tenant</H3>
      <Endpoint method="GET" path="/api/tenants/:id" />

      <H3>Create tenant</H3>
      <Endpoint method="POST" path="/api/tenants" />
      <Pre lang="request">{`{
  "name": "Acme Corp",
  "slug": "acme",
  "max_rpm": 1000,
  "max_tpm": 100000,
  "monthly_budget_usd": 500
}`}</Pre>

      <H3>Update tenant</H3>
      <Endpoint method="PUT" path="/api/tenants/:id" />

      <H3>Delete tenant</H3>
      <Endpoint method="DELETE" path="/api/tenants/:id" />

      <H2 id="users">Users</H2>

      <H3>List users in current tenant</H3>
      <Endpoint method="GET" path="/api/users" />

      <H3>Invite user</H3>
      <Endpoint method="POST" path="/api/users" />
      <Pre lang="request">{`{
  "email": "bob@acme.com",
  "role": "member",
  "password": "<initial>"
}`}</Pre>

      <H3>Update user</H3>
      <Endpoint method="PUT" path="/api/users/:id" />
      <Pre lang="request">{`{ "role": "admin", "status": "active" }`}</Pre>

      <H3>Delete user</H3>
      <Endpoint method="DELETE" path="/api/users/:id" />

      <H2 id="api-keys">API keys</H2>

      <H3>List keys</H3>
      <Endpoint method="GET" path="/api/api-keys" />

      <H3>Create key</H3>
      <Endpoint method="POST" path="/api/api-keys" />
      <Pre lang="request">{`{
  "name": "prod-backend",
  "user_id": "...",
  "rpm_limit": 60,
  "allowed_models": ["gpt-4o", "claude-sonnet-4-6"]
}`}</Pre>
      <Pre lang="response">{`{
  "id": "...",
  "name": "prod-backend",
  "key": "sg_live_abc...",   // shown only once
  "prefix": "sg_live_abc",
  "created_at": "..."
}`}</Pre>

      <H3>Rotate key</H3>
      <Endpoint method="POST" path="/api/api-keys/:id/rotate" />

      <H3>Delete key</H3>
      <Endpoint method="DELETE" path="/api/api-keys/:id" />

      <H2 id="backends">Backends</H2>

      <H3>List backends</H3>
      <Endpoint method="GET" path="/api/backends" />

      <H3>Create backend</H3>
      <Endpoint method="POST" path="/api/backends" />
      <Pre lang="request">{`{
  "name": "openai-primary",
  "kind": "open_ai",
  "base_url": "https://api.openai.com/v1",
  "api_key": "sk-...",       // encrypted at rest
  "models": ["gpt-4o", "gpt-4o-mini"]
}`}</Pre>

      <H3>Update backend</H3>
      <Endpoint method="PUT" path="/api/backends/:id" />

      <H3>Delete backend</H3>
      <Endpoint method="DELETE" path="/api/backends/:id" />

      <H3>Backend health</H3>
      <Endpoint method="GET" path="/api/backends/:id/health" />

      <H2 id="routes">Routes</H2>

      <H3>List routes</H3>
      <Endpoint method="GET" path="/api/routes" />

      <H3>Create route</H3>
      <Endpoint method="POST" path="/api/routes" />
      <Pre lang="request">{`{
  "model_pattern": "gpt-4o*",
  "backends": [
    { "id": "...", "weight": 80 },
    { "id": "...", "weight": 20 }
  ],
  "fallback": { "id": "..." }
}`}</Pre>

      <H2 id="prompts">Prompts</H2>

      <H3>List prompts</H3>
      <Endpoint method="GET" path="/api/prompts" />

      <H3>Create prompt</H3>
      <Endpoint method="POST" path="/api/prompts" />
      <Pre lang="request">{`{
  "key": "customer-support-v3",
  "template": "You are helping {{customer_name}} with a {{issue}} issue...",
  "variables": ["customer_name", "issue"]
}`}</Pre>

      <H3>Get prompt versions</H3>
      <Endpoint method="GET" path="/api/prompts/:key/versions" />

      <H3>Publish new version</H3>
      <Endpoint method="POST" path="/api/prompts/:key/versions" />

      <H3>Set canary</H3>
      <Endpoint method="POST" path="/api/prompts/:key/canary" />
      <Pre lang="request">{`{ "version": "4", "percentage": 10 }`}</Pre>

      <H2 id="guardrails">Guardrails</H2>

      <H3>List guardrails</H3>
      <Endpoint method="GET" path="/api/guardrails" />

      <H3>Create guardrail</H3>
      <Endpoint method="POST" path="/api/guardrails" />
      <Pre lang="request">{`{
  "name": "no-ssn",
  "kind": "regex",
  "stage": "post_call",
  "mode": "redact",
  "config": { "pattern": "\\\\d{3}-\\\\d{2}-\\\\d{4}" }
}`}</Pre>

      <H3>Test guardrail</H3>
      <Endpoint method="POST" path="/api/guardrails/:id/test" />
      <Pre lang="request">{`{ "text": "my ssn is 123-45-6789" }`}</Pre>

      <H2 id="mcp">MCP</H2>

      <H3>List MCP servers</H3>
      <Endpoint method="GET" path="/api/mcp/servers" />

      <H3>Register MCP server</H3>
      <Endpoint method="POST" path="/api/mcp/servers" />
      <Pre lang="request">{`{
  "name": "github",
  "transport": "streamable_http",
  "url": "https://mcp.github.com/rpc",
  "auth": { "type": "bearer", "token": "..." }
}`}</Pre>

      <H3>JSON-RPC endpoint</H3>
      <Endpoint method="POST" path="/mcp/v1/rpc" />
      <P>Standard MCP 2025-06-18 JSON-RPC 2.0. Common methods:</P>
      <UL>
        <li><Code>initialize</Code></li>
        <li><Code>tools/list</Code></li>
        <li><Code>tools/call</Code></li>
        <li><Code>resources/list</Code></li>
        <li><Code>prompts/list</Code></li>
      </UL>

      <H2 id="audit">Audit logs</H2>

      <H3>List audit events</H3>
      <Endpoint method="GET" path="/api/audit" />
      <P>
        Query parameters: <Code>user_id</Code>, <Code>action</Code>,{" "}
        <Code>from</Code>, <Code>to</Code>, <Code>limit</Code>, <Code>offset</Code>.
      </P>

      <H2 id="webhooks">Webhooks</H2>

      <H3>List webhooks</H3>
      <Endpoint method="GET" path="/api/webhooks" />

      <H3>Create webhook</H3>
      <Endpoint method="POST" path="/api/webhooks" />
      <Pre lang="request">{`{
  "url": "https://example.com/hook",
  "events": ["budget.threshold_reached", "audit.login_failed"],
  "secret": "whsec_..."
}`}</Pre>

      <H2 id="feedback">Feedback <Code>Pro+</Code></H2>
      <P>
        End-user feedback on LLM responses (thumbs-up/down/comments).
        Gated — returns <Code>402 feature_gated</Code> on Community plan.
      </P>

      <H3>Submit feedback</H3>
      <Endpoint method="POST" path="/api/v1/feedback" />
      <Pre lang="json">{`{
  "llm_log_id": "uuid",        // or "request_id": "..." — one is required
  "rating": 1,                  // -1 | 0 | 1
  "comment": "Helpful answer",
  "metadata": {}
}`}</Pre>

      <H3>List feedback</H3>
      <Endpoint method="GET" path="/api/v1/feedback?limit=50" />

      <H3>Stats (aggregate over window)</H3>
      <Endpoint method="GET" path="/api/v1/feedback/stats?days=30" />

      <H2 id="sso">SSO / OAuth2 <Code>Enterprise</Code></H2>
      <P>
        OAuth2 / OIDC with 5 providers: Keycloak, Okta, Google, GitHub, Microsoft Entra.
        All OIDC providers use PKCE S256 + CSRF state; GitHub uses its non-OIDC flow.
      </P>

      <H3>Start SSO flow (public, unauthenticated)</H3>
      <Endpoint method="GET" path="/api/v1/auth/sso/:slug/authorize?tenant=<slug>" />

      <H3>Provider callback</H3>
      <Endpoint method="GET" path="/api/v1/auth/sso/:slug/callback" />
      <P>Issues JWT access + refresh tokens; auto-provisions user when <Code>auto_provision=true</Code>.</P>

      <H3>List providers (TenantAdmin+)</H3>
      <Endpoint method="GET" path="/api/v1/sso/providers" />

      <H3>Create provider</H3>
      <Endpoint method="POST" path="/api/v1/sso/providers" />
      <Pre lang="json">{`{
  "kind": "keycloak",           // keycloak | okta | google | github | microsoft | oidc_generic
  "display_name": "Acme SSO",
  "slug": "acme",
  "client_id": "...",
  "client_secret": "...",
  "issuer_url": "https://kc.acme.com/realms/main",
  "scopes": "openid profile email",
  "default_role": "user",
  "auto_provision": true
}`}</Pre>

      <H3>Delete provider (soft-delete)</H3>
      <Endpoint method="DELETE" path="/api/v1/sso/providers/:id" />

      <H2 id="organizations">Organizations <Code>Enterprise</Code></H2>
      <P>Tenant-of-tenants grouping for billing, cross-tenant views, and multi-environment accounts. SuperAdmin only.</P>

      <H3>List organizations</H3>
      <Endpoint method="GET" path="/api/v1/organizations" />

      <H3>Create organization</H3>
      <Endpoint method="POST" path="/api/v1/organizations" />
      <Pre lang="json">{`{ "slug": "acme", "name": "Acme Corp", "plan": "enterprise", "metadata": {} }`}</Pre>

      <H3>List tenants under org / assign tenant</H3>
      <Endpoint method="GET" path="/api/v1/organizations/:id/tenants" />
      <Endpoint method="POST" path="/api/v1/organizations/:id/tenants/:tenant_id" />

      <H2 id="license">License &amp; Plan Tiers</H2>
      <P>Three plan tiers — features enabled per tier are returned by the <Code>/features</Code> endpoint.</P>

      <H3>Current activation state</H3>
      <Endpoint method="GET" path="/api/v1/license/status" />

      <H3>Feature matrix (drives upsell UI + nav)</H3>
      <Endpoint method="GET" path="/api/v1/license/features" />
      <Pre lang="json">{`{
  "plan": "professional",
  "features": {
    "plan": "professional",
    "max_requests_per_month": 100000,
    "logs_enabled": true,
    "feedback_enabled": true,
    "semantic_cache_enabled": true,
    "sso_enabled": false,
    "audit_logs_enabled": false,
    ...
  }
}`}</Pre>

      <H3>Feature-gated errors</H3>
      <P>
        When a handler refuses because the plan doesn't include a feature, the gateway returns
        <Code>402 Payment Required</Code>:
      </P>
      <Pre lang="json">{`{
  "error": {
    "code": "feature_gated",
    "message": "Feature 'sso' requires plan 'enterprise' or higher. Current plan: 'professional'.",
    "feature": "sso",
    "required_plan": "enterprise",
    "current_plan": "professional"
  }
}`}</Pre>

      <H2 id="settings">Settings</H2>

      <H3>Get tenant settings</H3>
      <Endpoint method="GET" path="/api/settings" />

      <H3>Update settings</H3>
      <Endpoint method="PUT" path="/api/settings" />

      <H2 id="health">Health &amp; observability</H2>

      <H3>Liveness</H3>
      <Endpoint method="GET" path="/healthz" />

      <H3>Readiness (DB + Redis)</H3>
      <Endpoint method="GET" path="/readyz" />

      <H3>Prometheus metrics</H3>
      <Endpoint method="GET" path="/metrics" />

      <H2 id="rate-limit-headers">Rate-limit headers</H2>
      <P>
        Every response from <Code>/v1/*</Code> includes:
      </P>
      <Pre lang="headers">{`X-RateLimit-Limit-Requests: 60
X-RateLimit-Remaining-Requests: 47
X-RateLimit-Reset-Requests: 12    (seconds)
X-RateLimit-Limit-Tokens: 100000
X-RateLimit-Remaining-Tokens: 83412`}</Pre>

      <H2 id="pagination">Pagination</H2>
      <P>
        Collection endpoints accept <Code>limit</Code> (default 50, max 200) and{" "}
        <Code>offset</Code>. Responses include:
      </P>
      <Pre lang="json">{`{
  "data": [...],
  "total": 1234,
  "limit": 50,
  "offset": 0
}`}</Pre>

      <H2 id="openapi">OpenAPI schema</H2>
      <P>
        A machine-readable OpenAPI 3.1 document is served at{" "}
        <Code>/openapi.json</Code>. Swagger UI is available at <Code>/docs/swagger</Code>{" "}
        when built with the <Code>swagger</Code> feature flag.
      </P>
    </div>
  )
}
