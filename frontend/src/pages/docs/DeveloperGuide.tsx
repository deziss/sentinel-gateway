import { H1, Lead, H2, H3, P, UL, Code, Pre, Callout } from "./_primitives"

export function DeveloperGuide() {
  return (
    <div>
      <H1>Developer Guide</H1>
      <Lead>
        Integrate your application with Sentinel Gateway using the OpenAI-compatible
        API surface, streaming, function calling, prompt refs, and MCP tools.
      </Lead>

      <H2 id="base-url">Base URL &amp; authentication</H2>
      <P>
        The gateway exposes an OpenAI-compatible surface at{" "}
        <Code>/v1/chat/completions</Code>. Point any OpenAI SDK at your gateway:
      </P>
      <Pre lang="env">{`OPENAI_BASE_URL=https://gateway.example.com/v1
OPENAI_API_KEY=sg_live_...`}</Pre>
      <Callout kind="info">
        The prefix <Code>sg_</Code> distinguishes Sentinel keys from provider keys. Keys
        are checked against SHA-256 hashes — the raw key is never stored.
      </Callout>

      <H2 id="quickstart">Quickstart — Python</H2>
      <Pre lang="python">{`from openai import OpenAI

client = OpenAI(
    base_url="https://gateway.example.com/v1",
    api_key="sg_live_...",
)

resp = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello, world!"}],
)
print(resp.choices[0].message.content)`}</Pre>

      <H3>Node / TypeScript</H3>
      <Pre lang="typescript">{`import OpenAI from "openai"

const client = new OpenAI({
  baseURL: "https://gateway.example.com/v1",
  apiKey: process.env.SENTINEL_KEY,
})

const resp = await client.chat.completions.create({
  model: "claude-sonnet-4-6",
  messages: [{ role: "user", content: "Hello" }],
})
console.log(resp.choices[0].message.content)`}</Pre>

      <H3>curl</H3>
      <Pre lang="bash">{`curl https://gateway.example.com/v1/chat/completions \\
  -H "Authorization: Bearer sg_live_..." \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Hello"}]
  }'`}</Pre>

      <H2 id="streaming">Streaming</H2>
      <P>
        Set <Code>stream: true</Code>. The gateway proxies Server-Sent Events from the
        upstream provider with the same <Code>data: {"{...}"}</Code> framing.
      </P>
      <Pre lang="python">{`stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Explain streaming"}],
    stream=True,
)
for chunk in stream:
    print(chunk.choices[0].delta.content or "", end="", flush=True)`}</Pre>

      <H2 id="model-routing">Model routing</H2>
      <P>
        You pick the model; the gateway picks the backend. Your tenant admin has
        configured routes that say, for example, "<Code>gpt-4o</Code> → openai-primary
        with openai-backup as fallback." You never need to know which backend served
        a given request, but you can see it on the response headers:
      </P>
      <Pre lang="headers">{`x-sentinel-backend-id: openai-primary
x-sentinel-request-id: 01HQZ8...
x-sentinel-cost-usd: 0.00384
x-sentinel-tokens-in: 128
x-sentinel-tokens-out: 256
x-sentinel-latency-ms: 2340`}</Pre>

      <H2 id="prompt-refs">Prompt references</H2>
      <P>
        Instead of hardcoding prompts, reference a versioned template registered in
        the <strong>Prompts</strong> dashboard:
      </P>
      <Pre lang="json">{`{
  "model": "gpt-4o",
  "prompt_ref": { "key": "customer-support-v3", "version": "latest" },
  "variables": { "customer_name": "Alice", "issue": "billing" }
}`}</Pre>
      <P>
        The gateway expands the template (with <Code>{"{{variable}}"}</Code> interpolation)
        before forwarding the request. Use <Code>"version": "latest"</Code> to pick up
        updates automatically, or pin a specific version for stability.
      </P>

      <H2 id="function-calling">Function calling / tools</H2>
      <P>
        Standard OpenAI <Code>tools</Code> / <Code>tool_choice</Code> fields are
        passed through unchanged. For Anthropic-style tool use, the gateway
        translates between formats when <Code>anthropic</Code> is the upstream.
      </P>

      <H2 id="mcp">MCP (Model Context Protocol)</H2>
      <P>
        Sentinel acts as both an MCP <strong>server</strong> (to your agent) and an MCP{" "}
        <strong>client</strong> (to upstream tool servers). Point your MCP client at the
        gateway's MCP endpoint:
      </P>
      <Pre lang="http">{`POST /mcp/v1/rpc
Authorization: Bearer sg_live_...
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list"
}`}</Pre>
      <P>
        The response returns tools from every connected upstream, namespaced by
        server:
      </P>
      <Pre lang="json">{`{
  "tools": [
    { "name": "github__create_issue", ... },
    { "name": "slack__post_message", ... },
    { "name": "filesystem__read_file", ... }
  ]
}`}</Pre>

      <H2 id="errors">Error handling</H2>
      <P>The gateway returns standard HTTP codes with JSON bodies:</P>
      <Pre lang="json">{`{
  "error": {
    "type": "rate_limit_exceeded",
    "message": "RPM exceeded for key; retry after 12s",
    "retry_after": 12
  }
}`}</Pre>
      <P>Common codes:</P>
      <UL>
        <li><Code>401</Code> — invalid or missing API key</li>
        <li><Code>402</Code> — tenant budget exceeded</li>
        <li><Code>403</Code> — model not allowed for this key/tenant</li>
        <li><Code>429</Code> — rate-limited (check <Code>Retry-After</Code> header)</li>
        <li><Code>502</Code> — upstream provider error</li>
        <li><Code>504</Code> — upstream timed out</li>
      </UL>
      <P>
        Retries are automatic for idempotent upstream failures (5xx) with
        exponential backoff up to the route's configured retry budget.
      </P>

      <H2 id="idempotency">Idempotency</H2>
      <P>
        Send <Code>Idempotency-Key: &lt;uuid&gt;</Code> to dedupe retries. The gateway
        caches successful responses by key for 10 minutes.
      </P>

      <H2 id="observability">Observability</H2>
      <P>Every request emits:</P>
      <UL>
        <li>
          An OpenTelemetry span with trace ID (propagated to upstream via{" "}
          <Code>traceparent</Code>)
        </li>
        <li>
          Prometheus metrics:{" "}
          <Code>gateway_proxy_requests_total</Code>,{" "}
          <Code>gateway_tokens_total</Code>, <Code>gateway_cost_usd_total</Code>
        </li>
        <li>A structured JSON log line</li>
        <li>Optional Langfuse / Helicone export if configured by your tenant admin</li>
      </UL>
      <P>
        Pass your own <Code>traceparent</Code> header to link gateway spans to your
        application's trace.
      </P>

      <H2 id="webhooks">Webhooks</H2>
      <P>
        Your tenant admin can register webhook endpoints for budget alerts, audit
        events, or guardrail flags. Payloads are signed with HMAC-SHA256 using a
        shared secret:
      </P>
      <Pre lang="headers">{`X-Sentinel-Event: budget.threshold_reached
X-Sentinel-Signature: sha256=<hex hmac>
X-Sentinel-Timestamp: 1712345678`}</Pre>
      <P>
        Verify by computing <Code>HMAC(secret, timestamp + "." + body)</Code> and
        comparing constant-time against the signature header. Reject events older
        than 5 minutes.
      </P>

      <H2 id="best-practices">Best practices</H2>
      <UL>
        <li>
          <strong>Use one key per service.</strong> Makes rotation surgical and the audit
          log readable.
        </li>
        <li>
          <strong>Set a reasonable <Code>max_tokens</Code></strong>. Prevents run-away
          generations from eating your budget.
        </li>
        <li>
          <strong>Prefer <Code>prompt_ref</Code> over inline prompts.</strong> You get
          versioning, review, and A/B testing for free.
        </li>
        <li>
          <strong>Attach a <Code>user</Code> field</strong> — it flows into analytics and
          helps diagnose per-user problems.
        </li>
        <li>
          <strong>Check <Code>finish_reason</Code></strong>, not just the text. A{" "}
          <Code>content_filter</Code> stop means a guardrail fired.
        </li>
        <li>
          <strong>Propagate <Code>traceparent</Code></strong> from your app so traces span
          your service → gateway → provider.
        </li>
      </UL>

      <H2 id="local-dev">Local development</H2>
      <P>The gateway ships a docker-compose for local bring-up:</P>
      <Pre lang="bash">{`git clone https://github.com/your-org/sentinel-gateway
cd sentinel-gateway
docker compose up -d
# backend: http://localhost:8080
# frontend: http://localhost:3005
# docs (this site): http://localhost:3005/docs`}</Pre>
      <P>
        Seed a test tenant with the first-run bootstrap flow, then copy an API key and
        point your SDK at <Code>http://localhost:8080/v1</Code>.
      </P>
    </div>
  )
}
