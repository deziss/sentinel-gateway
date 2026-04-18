import { Link } from "react-router-dom"
import { User, Building2, Code2, Terminal, ArrowRight } from "lucide-react"
import { H1, Lead, H2, P, UL } from "./_primitives"

const cards = [
  {
    to: "/docs/users",
    title: "User Guide",
    icon: User,
    desc: "For end users — sign in, use the Playground, understand your quotas, manage your profile.",
  },
  {
    to: "/docs/tenants",
    title: "Tenant Admin Guide",
    icon: Building2,
    desc: "For tenant admins — manage backends, API keys, users, budgets, guardrails, prompts, and audit logs.",
  },
  {
    to: "/docs/developers",
    title: "Developer Guide",
    icon: Code2,
    desc: "For app developers — integrate your application with the gateway's OpenAI-compatible API.",
  },
  {
    to: "/docs/api",
    title: "API Reference",
    icon: Terminal,
    desc: "Full REST API — authentication, LLM proxy, tenants, users, keys, prompts, guardrails, MCP.",
  },
]

export function DocsIndex() {
  return (
    <div>
      <H1>Sentinel Gateway Documentation</H1>
      <Lead>
        Sentinel is a universal AI/LLM gateway: a single OpenAI-compatible endpoint
        routes requests across multiple providers with rate limiting, cost tracking,
        guardrails, prompt management, and full observability.
      </Lead>

      <div className="grid gap-4 sm:grid-cols-2">
        {cards.map((c) => (
          <Link
            key={c.to}
            to={c.to}
            className="group rounded-lg border bg-background p-5 hover:border-primary hover:shadow-sm transition-all"
          >
            <div className="flex items-start gap-3">
              <div className="size-9 rounded-md bg-primary/10 text-primary flex items-center justify-center">
                <c.icon className="size-5" />
              </div>
              <div className="flex-1">
                <div className="flex items-center justify-between">
                  <div className="font-semibold">{c.title}</div>
                  <ArrowRight className="size-4 opacity-0 group-hover:opacity-100 transition-opacity" />
                </div>
                <p className="text-sm text-muted-foreground mt-1 leading-6">
                  {c.desc}
                </p>
              </div>
            </div>
          </Link>
        ))}
      </div>

      <H2>What is Sentinel Gateway?</H2>
      <P>
        Sentinel sits between your applications and LLM providers (OpenAI,
        Anthropic, Google, Mistral, local models, and more). It provides a single
        unified API surface and centralizes everything your team cares about:
      </P>
      <UL>
        <li>
          <strong>Multi-provider routing</strong> — route by model name, tenant, cost,
          or inference-server load.
        </li>
        <li>
          <strong>Rate limiting &amp; budgets</strong> — per-key RPM, per-tenant TPM,
          monthly cost caps, and CEL-weighted rules.
        </li>
        <li>
          <strong>Guardrails</strong> — regex, PII, length, and JSON-schema checks
          applied pre-call or post-call.
        </li>
        <li>
          <strong>Prompt management</strong> — versioned templates, A/B tests, rollbacks.
        </li>
        <li>
          <strong>MCP gateway</strong> — proxy Model Context Protocol tools to your
          agents with namespacing and auth.
        </li>
        <li>
          <strong>Observability</strong> — OpenTelemetry traces, Prometheus metrics,
          structured logs, optional Langfuse/Helicone export.
        </li>
      </UL>

      <H2>Choosing a guide</H2>
      <UL>
        <li>
          You use the Playground or consume models in your app —{" "}
          <Link to="/docs/users" className="text-primary underline">User Guide</Link>.
        </li>
        <li>
          You administer a tenant (backends, keys, budgets) —{" "}
          <Link to="/docs/tenants" className="text-primary underline">Tenant Admin Guide</Link>.
        </li>
        <li>
          You're wiring Sentinel into code —{" "}
          <Link to="/docs/developers" className="text-primary underline">Developer Guide</Link>.
        </li>
        <li>
          You need endpoint-by-endpoint reference —{" "}
          <Link to="/docs/api" className="text-primary underline">API Reference</Link>.
        </li>
      </UL>
    </div>
  )
}
