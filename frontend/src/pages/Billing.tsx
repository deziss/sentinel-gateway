import { Check, X, Crown, Rocket, Box } from "lucide-react"
import { usePlan } from "@/hooks/use-plan"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { PlanTier } from "@/lib/api"

type CellValue = boolean | string
type Row = { label: string; community: CellValue; professional: CellValue; enterprise: CellValue }
type Section = { title: string; rows: Row[] }

const MATRIX: Section[] = [
  {
    title: "Overview",
    rows: [
      { label: "Requests per Month", community: "No Limit (self-host)", professional: "100K", enterprise: "Custom" },
      { label: "Retention Period", community: "—", professional: "30 Days", enterprise: "Custom" },
    ],
  },
  {
    title: "AI Gateway",
    rows: [
      { label: "Universal API", community: true, professional: true, enterprise: true },
      { label: "Automatic Fallbacks", community: true, professional: true, enterprise: true },
      { label: "Load balancing", community: true, professional: true, enterprise: true },
      { label: "Conditional Routing", community: true, professional: true, enterprise: true },
      { label: "Automatic Retries", community: true, professional: true, enterprise: true },
      { label: "Request Timeouts", community: true, professional: true, enterprise: true },
      { label: "Config Management", community: false, professional: true, enterprise: true },
      { label: "LLM Key Management", community: false, professional: true, enterprise: "Budgeting/Rate Limits" },
      { label: "Simple Caching", community: false, professional: "Unlimited TTL", enterprise: "Unlimited TTL" },
      { label: "Semantic Caching", community: false, professional: true, enterprise: true },
      { label: "Unified Fine-Tuning/Batch APIs", community: false, professional: false, enterprise: true },
      { label: "AWS/GCP/Azure Private LLM", community: false, professional: false, enterprise: true },
    ],
  },
  {
    title: "Observability",
    rows: [
      { label: "Logs", community: false, professional: true, enterprise: true },
      { label: "Traces", community: false, professional: true, enterprise: true },
      { label: "Feedback", community: false, professional: true, enterprise: true },
      { label: "Custom Metadata", community: false, professional: true, enterprise: true },
      { label: "Filters", community: false, professional: true, enterprise: true },
      { label: "Alerts", community: false, professional: true, enterprise: true },
      { label: "FinOps + Executive Dashboard", community: false, professional: false, enterprise: true },
    ],
  },
  {
    title: "Prompt Management",
    rows: [
      { label: "Prompt Templates", community: false, professional: "Unlimited", enterprise: "Unlimited" },
      { label: "Playground", community: false, professional: true, enterprise: true },
      { label: "API Deployment", community: false, professional: true, enterprise: true },
      { label: "Versioning", community: false, professional: true, enterprise: true },
      { label: "Variable Management", community: false, professional: true, enterprise: true },
      { label: "Prompt Partials", community: false, professional: true, enterprise: true },
      { label: "Side-by-Side Comparison", community: false, professional: true, enterprise: true },
      { label: "User Access Control", community: false, professional: true, enterprise: true },
    ],
  },
  {
    title: "Guardrails",
    rows: [
      { label: "Deterministic Guardrails", community: false, professional: true, enterprise: true },
      { label: "Partner Guardrails", community: false, professional: true, enterprise: true },
      { label: "PII / PHI Redaction", community: false, professional: true, enterprise: true },
    ],
  },
  {
    title: "Security & Compliance",
    rows: [
      { label: "Role-Based Access Control", community: false, professional: true, enterprise: "Advanced" },
      { label: "Team Management", community: false, professional: true, enterprise: "Advanced" },
      { label: "Audit Logs", community: false, professional: false, enterprise: true },
      { label: "Admin APIs", community: false, professional: false, enterprise: true },
      { label: "SCIM Provisioning", community: false, professional: false, enterprise: true },
      { label: "JWT-based Authentication", community: false, professional: false, enterprise: true },
      { label: "Bring Your Own Key (BYOK)", community: false, professional: false, enterprise: true },
      { label: "SSO (Okta, Keycloak, Google, GitHub, Microsoft)", community: false, professional: false, enterprise: true },
      { label: "Compliance Certs (SOC2/GDPR)", community: false, professional: false, enterprise: true },
      { label: "BAA Signing", community: false, professional: false, enterprise: true },
      { label: "VPC Managed Hosting", community: false, professional: false, enterprise: true },
      { label: "Private Tenancy", community: false, professional: false, enterprise: true },
      { label: "Configurable Retention", community: false, professional: false, enterprise: true },
      { label: "Datalake Exports", community: false, professional: false, enterprise: true },
      { label: "Org Management", community: false, professional: false, enterprise: true },
    ],
  },
]

function cell(value: CellValue) {
  if (value === true) return <Check className="h-4 w-4 text-emerald-600 mx-auto" />
  if (value === false) return <X className="h-4 w-4 text-muted-foreground/40 mx-auto" />
  return <span className="text-xs text-muted-foreground">{value}</span>
}

export function Billing() {
  const { plan } = usePlan()

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Plans & Billing</h1>
        <p className="text-muted-foreground">
          Current plan: <strong>{planLabel(plan)}</strong>. Upgrade to unlock more features.
        </p>
      </div>

      {/* ── Plan cards ─────────────────────────────────── */}
      <div className="grid gap-4 md:grid-cols-3">
        <PlanCard
          tier="community"
          label="Open Source"
          icon={Box}
          price="Free"
          cta="Run Locally"
          current={plan === "community"}
          highlights={["Core gateway routing", "Self-hosted, no limits", "Universal LLM API", "Load balancing & fallbacks"]}
        />
        <PlanCard
          tier="professional"
          label="Professional"
          icon={Rocket}
          price="$49/mo"
          cta="Upgrade"
          current={plan === "professional"}
          featured
          highlights={[
            "100K requests/month",
            "Full observability (logs, traces)",
            "Prompt management + playground",
            "Guardrails + PII redaction",
            "RBAC + team management",
          ]}
        />
        <PlanCard
          tier="enterprise"
          label="Enterprise"
          icon={Crown}
          price="Custom"
          cta="Book a Call"
          current={plan === "enterprise"}
          highlights={[
            "Everything in Pro",
            "SSO, audit logs, SCIM, BYOK",
            "Datalake exports, org management",
            "VPC hosting, private tenancy",
            "SOC2 / GDPR / BAA",
          ]}
        />
      </div>

      {/* ── Full feature matrix ─────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle>Feature matrix</CardTitle>
          <CardDescription>Detailed feature comparison across plans.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2 pr-4 font-medium">Feature</th>
                  <th className="text-center py-2 px-4 font-medium">Open Source</th>
                  <th className="text-center py-2 px-4 font-medium">Professional</th>
                  <th className="text-center py-2 px-4 font-medium">Enterprise</th>
                </tr>
              </thead>
              <tbody>
                {MATRIX.map((section) => (
                  <>
                    <tr key={section.title} className="border-b bg-muted/30">
                      <td colSpan={4} className="py-2 pr-4 font-semibold text-xs uppercase tracking-wider text-muted-foreground">
                        {section.title}
                      </td>
                    </tr>
                    {section.rows.map((row) => (
                      <tr key={row.label} className="border-b hover:bg-muted/20">
                        <td className="py-2 pr-4">{row.label}</td>
                        <td className="text-center py-2 px-4">{cell(row.community)}</td>
                        <td className="text-center py-2 px-4">{cell(row.professional)}</td>
                        <td className="text-center py-2 px-4">{cell(row.enterprise)}</td>
                      </tr>
                    ))}
                  </>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

function planLabel(p: PlanTier | undefined): string {
  if (p === "community") return "Open Source"
  if (p === "professional") return "Professional"
  if (p === "enterprise") return "Enterprise"
  return "—"
}

interface PlanCardProps {
  tier: PlanTier
  label: string
  icon: React.ComponentType<{ className?: string }>
  price: string
  cta: string
  current: boolean
  featured?: boolean
  highlights: string[]
}

function PlanCard({ tier: _tier, label, icon: Icon, price, cta, current, featured, highlights }: PlanCardProps) {
  return (
    <Card className={featured ? "border-primary ring-1 ring-primary/30" : undefined}>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="flex items-center gap-2">
            <Icon className="h-5 w-5" /> {label}
          </CardTitle>
          {current && <Badge variant="success">Current</Badge>}
        </div>
        <p className="text-2xl font-bold pt-2">{price}</p>
      </CardHeader>
      <CardContent className="space-y-4">
        <ul className="space-y-2 text-sm">
          {highlights.map((h) => (
            <li key={h} className="flex items-start gap-2">
              <Check className="h-4 w-4 text-emerald-600 mt-0.5 flex-shrink-0" />
              <span>{h}</span>
            </li>
          ))}
        </ul>
        <Button
          className="w-full"
          variant={featured ? "default" : "outline"}
          disabled={current}
        >
          {current ? "Current Plan" : cta}
        </Button>
      </CardContent>
    </Card>
  )
}
