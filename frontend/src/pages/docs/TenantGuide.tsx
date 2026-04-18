import { H1, Lead, H2, H3, P, UL, OL, Code, Pre, Callout, Table, THead, TH, TR, TD } from "./_primitives"

export function TenantGuide() {
  return (
    <div>
      <H1>Tenant Admin Guide</H1>
      <Lead>
        For tenant administrators — configure backends, issue API keys, invite users,
        set budgets and guardrails, manage prompts, and audit activity.
      </Lead>

      <H2 id="roles">Roles &amp; permissions</H2>
      <Table>
        <THead>
          <TR>
            <TH>Role</TH>
            <TH>Can do</TH>
          </TR>
        </THead>
        <tbody>
          <TR>
            <TD><Code>owner</Code></TD>
            <TD>Everything — including changing billing and deleting the tenant.</TD>
          </TR>
          <TR>
            <TD><Code>admin</Code></TD>
            <TD>Manage backends, keys, users, budgets, guardrails, prompts, audit logs.</TD>
          </TR>
          <TR>
            <TD><Code>member</Code></TD>
            <TD>Use the Playground and analytics; cannot change configuration.</TD>
          </TR>
          <TR>
            <TD><Code>viewer</Code></TD>
            <TD>Read-only access to dashboards and analytics.</TD>
          </TR>
        </tbody>
      </Table>

      <H2 id="backends">Backends (upstream providers)</H2>
      <P>
        A <strong>backend</strong> is an upstream LLM provider your tenant can route to.
        The gateway ships with built-in support for OpenAI, Anthropic, Google, Mistral,
        Cohere, Groq, Together, DeepInfra, Fireworks, local (Ollama, vLLM, TGI), and
        any OpenAI-compatible endpoint.
      </P>

      <H3>Adding a backend</H3>
      <OL>
        <li>Go to <strong>Backends</strong> → <strong>New Backend</strong>.</li>
        <li>Choose a kind (e.g., <Code>open_ai</Code>, <Code>anthropic</Code>, <Code>ollama</Code>).</li>
        <li>Paste the provider's API key — it's encrypted at rest (ChaCha20-Poly1305).</li>
        <li>Optionally set a custom base URL (for self-hosted or Azure).</li>
        <li>Save and the backend is available to routes immediately.</li>
      </OL>

      <Callout kind="tip">
        Provider credentials are encrypted with ChaCha20-Poly1305 using the gateway's
        encryption key. The plaintext never appears in the UI after creation or in logs.
      </Callout>

      <H3>Routes (model-to-backend mapping)</H3>
      <P>
        A <strong>route</strong> tells the gateway: "when someone asks for model X, send the
        request to backend Y." Routes are matched by model name (exact or prefix)
        and can define priorities, weights, and fallbacks.
      </P>
      <Pre lang="example">{`model: gpt-4o
backends:
  - openai-primary  (weight 80)
  - openai-backup   (weight 20)
fallback: anthropic-claude  (on 5xx)`}</Pre>

      <H2 id="api-keys">API keys</H2>
      <P>
        API keys authenticate external applications. They start with the <Code>sg_</Code> prefix
        and are stored as SHA-256 hashes — Sentinel never stores the raw key.
      </P>
      <OL>
        <li>Go to <strong>API Keys</strong> → <strong>New Key</strong>.</li>
        <li>Give it a name and pick the user who owns it.</li>
        <li>Optionally set RPM/TPM overrides and model allowlists.</li>
        <li>
          Copy the key that's shown <em>once</em>. If you lose it, rotate — you cannot
          retrieve it later.
        </li>
      </OL>
      <Callout kind="warn">
        Treat API keys like passwords. Rotate immediately on suspected leak. The audit
        log records every use of every key.
      </Callout>

      <H2 id="users">Users</H2>
      <H3>Inviting a user</H3>
      <OL>
        <li>Go to <strong>Users</strong> → <strong>Invite</strong>.</li>
        <li>Enter email and pick a role.</li>
        <li>The user receives credentials and is prompted to change the password on first login.</li>
      </OL>
      <H3>Disabling a user</H3>
      <P>
        Toggle the user's status to <Code>disabled</Code>. Their sessions are invalidated
        at next token refresh and their API keys stop working immediately.
      </P>

      <H2 id="budgets">Budgets &amp; rate limits</H2>
      <P>Sentinel enforces limits at three levels:</P>
      <Table>
        <THead>
          <TR>
            <TH>Scope</TH>
            <TH>Knob</TH>
            <TH>Unit</TH>
          </TR>
        </THead>
        <tbody>
          <TR>
            <TD>Per API key</TD>
            <TD>RPM</TD>
            <TD>requests / minute</TD>
          </TR>
          <TR>
            <TD>Per tenant</TD>
            <TD>TPM</TD>
            <TD>tokens / minute</TD>
          </TR>
          <TR>
            <TD>Per tenant</TD>
            <TD>Monthly budget</TD>
            <TD>USD</TD>
          </TR>
          <TR>
            <TD>Any scope</TD>
            <TD>CEL rule</TD>
            <TD>custom (e.g., cost-weighted)</TD>
          </TR>
        </tbody>
      </Table>
      <P>
        CEL rules let you write expressions like{" "}
        <Code>cost.usd &gt; 0.10 && model.startsWith("gpt-4")</Code> to throttle expensive
        premium calls while leaving cheap ones fast.
      </P>

      <H2 id="guardrails">Guardrails</H2>
      <P>
        Guardrails are a pluggable pipeline that inspects requests and responses.
        Configure them under <strong>Guardrails</strong>:
      </P>
      <Table>
        <THead>
          <TR>
            <TH>Kind</TH>
            <TH>What it does</TH>
          </TR>
        </THead>
        <tbody>
          <TR>
            <TD><Code>regex</Code></TD>
            <TD>Match or block by pattern (e.g., forbidden keywords).</TD>
          </TR>
          <TR>
            <TD><Code>pii</Code></TD>
            <TD>Detect/redact emails, phone numbers, SSNs, credit cards.</TD>
          </TR>
          <TR>
            <TD><Code>length</Code></TD>
            <TD>Enforce min/max prompt or completion length.</TD>
          </TR>
          <TR>
            <TD><Code>json_schema</Code></TD>
            <TD>Validate completions against a JSON schema for structured outputs.</TD>
          </TR>
        </tbody>
      </Table>
      <P>
        Each guardrail runs at a stage (<Code>pre_call</Code>, <Code>post_call</Code>, or{" "}
        <Code>logging_only</Code>) and operates in one of three modes:{" "}
        <Code>block</Code>, <Code>redact</Code>, or <Code>flag</Code>.
      </P>

      <H2 id="prompts">Prompt management</H2>
      <P>
        The <strong>Prompts</strong> page gives your team a versioned, reviewable
        prompt registry. Instead of hardcoding prompts in application code, reference
        them by key:
      </P>
      <Pre lang="chat/completions body">{`{
  "model": "gpt-4o",
  "prompt_ref": { "key": "customer-support-v3", "version": "latest" },
  "variables": { "customer_name": "Alice", "issue": "billing" }
}`}</Pre>
      <P>
        Templates support <Code>{"{{variable}}"}</Code> interpolation. Use{" "}
        <strong>canary</strong> deploys to route a percentage of traffic to a new version.
      </P>

      <H2 id="mcp">MCP servers</H2>
      <P>
        <strong>MCP Servers</strong> lets you proxy Model Context Protocol tool servers
        (e.g., GitHub, Slack, filesystem) to your agents with namespacing and auth.
      </P>
      <OL>
        <li>Register an MCP server by URL (stdio or streamable HTTP).</li>
        <li>Pick a namespace — tools get prefixed (<Code>github__create_issue</Code>).</li>
        <li>Agents discover the combined tool list via the gateway's MCP endpoint.</li>
      </OL>

      <H2 id="audit">Audit logs</H2>
      <P>
        Every sensitive operation is recorded: logins, failed logins, key rotations,
        user invites, role changes, backend edits, guardrail changes, budget
        increases, and each LLM request (with tenant/user/model/tokens/cost).
      </P>
      <P>Filter by user, action, date range. Export to CSV for compliance reviews.</P>

      <H2 id="settings">Settings</H2>
      <P>Under <strong>Settings</strong> you can manage:</P>
      <UL>
        <li>Tenant name and default model</li>
        <li>Monthly budget and alert thresholds</li>
        <li>Webhook endpoints (budget alerts, audit events)</li>
        <li>Optional Langfuse / Helicone export credentials</li>
        <li>CORS origins (production) and security headers</li>
      </UL>

      <H2 id="operations">Operational tips</H2>
      <UL>
        <li>
          <strong>Rotate keys quarterly.</strong> The audit log makes this painless —
          create new, switch clients, delete old.
        </li>
        <li>
          <strong>Use canary routes</strong> when trying a new provider; send 5% of
          traffic, watch error rate in Analytics, ramp up.
        </li>
        <li>
          <strong>Set budget alerts at 70% / 90%</strong> so you're not surprised on
          the first of the month.
        </li>
        <li>
          <strong>Watch the dashboard after guardrail changes</strong> — a too-strict
          regex can start blocking legitimate traffic.
        </li>
      </UL>
    </div>
  )
}
