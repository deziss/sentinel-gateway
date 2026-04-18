import { H1, Lead, H2, H3, P, UL, OL, Code, Pre, Callout, Table, THead, TH, TR, TD } from "./_primitives"

export function UserGuide() {
  return (
    <div>
      <H1>User Guide</H1>
      <Lead>
        For end users of the Sentinel Gateway dashboard — signing in, exploring models,
        running prompts, and understanding your limits and costs.
      </Lead>

      <H2 id="sign-in">Signing in</H2>
      <P>
        Your tenant admin will invite you with an email. You'll receive an initial
        password that you'll be prompted to change on first login.
      </P>
      <OL>
        <li>Open the gateway URL you were given.</li>
        <li>Enter your email and password on the <Code>/login</Code> screen.</li>
        <li>You'll land on the Dashboard — your account is scoped to a single tenant.</li>
      </OL>
      <Callout kind="info">
        Sessions use short-lived access tokens (15 minutes) with refresh tokens (7 days).
        You'll be re-authenticated in the background while you work.
      </Callout>

      <H2 id="dashboard">Dashboard overview</H2>
      <P>
        The Dashboard shows a live snapshot of your tenant's activity — recent
        requests, error rate, token usage, and cost — over a rolling window. Numbers
        update every few seconds.
      </P>

      <H2 id="playground">LLM Playground</H2>
      <P>
        The <strong>Playground</strong> is a no-code way to try any model you have access to.
      </P>
      <OL>
        <li>Pick a model from the catalog (e.g., <Code>gpt-4o</Code>, <Code>claude-sonnet-4-6</Code>).</li>
        <li>Type a prompt in the chat area.</li>
        <li>Optionally adjust temperature, max tokens, and system prompt.</li>
        <li>Click <strong>Send</strong> — the response streams back in real time.</li>
      </OL>
      <P>
        Each Playground call is billed and audited exactly like an API call, counts
        toward your rate limits and budgets, and shows up in Analytics.
      </P>

      <H2 id="model-catalog">Model Catalog</H2>
      <P>
        The <strong>Model Catalog</strong> page lists every model available to you, with:
      </P>
      <UL>
        <li>Provider (OpenAI, Anthropic, Google, local, ...)</li>
        <li>Input / output cost per 1M tokens</li>
        <li>Context window size</li>
        <li>Capabilities (vision, function calling, streaming)</li>
      </UL>

      <H2 id="analytics">Analytics</H2>
      <P>
        The <strong>LLM Analytics</strong> page shows usage over time, broken down by:
      </P>
      <UL>
        <li>Model — which models you use most</li>
        <li>User — who in your tenant is calling what</li>
        <li>Cost — running total vs. your tenant's budget</li>
        <li>Errors — rate and reasons (rate-limited, provider down, ...)</li>
      </UL>

      <H2 id="limits">Understanding your limits</H2>
      <P>Each request passes through several gates. In order:</P>
      <Table>
        <THead>
          <TR>
            <TH>Check</TH>
            <TH>If exceeded</TH>
          </TR>
        </THead>
        <tbody>
          <TR>
            <TD>Authentication (valid session / API key)</TD>
            <TD>401 Unauthorized</TD>
          </TR>
          <TR>
            <TD>Tenant license</TD>
            <TD>403 Forbidden</TD>
          </TR>
          <TR>
            <TD>Per-key rate limit (RPM)</TD>
            <TD>429 Too Many Requests</TD>
          </TR>
          <TR>
            <TD>Tenant TPM (tokens per minute)</TD>
            <TD>429 Too Many Requests</TD>
          </TR>
          <TR>
            <TD>Monthly tenant budget</TD>
            <TD>402 Payment Required</TD>
          </TR>
          <TR>
            <TD>Guardrails (regex / PII / length / schema)</TD>
            <TD>400 or redacted response</TD>
          </TR>
        </tbody>
      </Table>
      <P>
        If you hit a limit, the response body tells you exactly which one and when
        you can retry. Contact your tenant admin to raise the ceiling.
      </P>

      <H2 id="account">Your profile and password</H2>
      <P>
        The <strong>user menu</strong> (top-right of the app) lets you change your password
        and sign out. Passwords are hashed with Argon2id — nobody, including the
        admin, can see your plaintext.
      </P>

      <H3>Forgot password</H3>
      <P>
        Ask your tenant admin to reset it — they can issue a temporary password
        that you'll be forced to change at next login.
      </P>

      <H2 id="troubleshooting">Troubleshooting</H2>
      <H3>"Rate limit exceeded"</H3>
      <P>
        You've sent too many requests in the last minute. Wait, space out requests,
        or ask your admin for a higher limit on your key.
      </P>
      <H3>"Budget exceeded"</H3>
      <P>
        Your tenant has hit its monthly cost cap. Only an admin can raise it.
      </P>
      <H3>"Model unavailable"</H3>
      <P>
        The upstream provider may be down — check the Backends status page if you
        have access, or ask your admin. Retries are automatic for most errors.
      </P>
      <H3>Streaming response stopped</H3>
      <P>
        If a response cuts off, check <strong>finish_reason</strong> in the UI footer:
      </P>
      <Pre lang="values">{`length    — hit max_tokens; raise it and retry
stop      — normal completion
content_filter — guardrail triggered
tool_calls — model wants to call a tool (if using MCP)`}</Pre>

      <H2 id="privacy">Privacy &amp; security</H2>
      <UL>
        <li>
          <strong>No plaintext passwords.</strong> All authentication uses Argon2id hashes.
        </li>
        <li>
          <strong>API keys are never shown twice.</strong> Copy the key when it's created; if
          you lose it, rotate it.
        </li>
        <li>
          <strong>Prompts and completions are logged</strong> for auditing. Your admin
          controls retention.
        </li>
        <li>
          <strong>PII guardrails</strong> may redact content in responses if configured —
          this is tenant-controlled policy.
        </li>
      </UL>
    </div>
  )
}
