import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor } from "@testing-library/react"
import { BrowserRouter } from "react-router-dom"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { FeatureGate } from "./FeatureGate"
import * as api from "@/lib/api"
import type { FeaturesResponse } from "@/lib/api"

vi.mock("@/lib/api", async () => {
  const mod = await vi.importActual<typeof import("@/lib/api")>("@/lib/api")
  return { ...mod, getFeatures: vi.fn() }
})

const mockApi = api as unknown as { getFeatures: ReturnType<typeof vi.fn> }

function renderGated(ui: React.ReactElement) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={qc}>
      <BrowserRouter>{ui}</BrowserRouter>
    </QueryClientProvider>
  )
}

function featuresFor(plan: "community" | "professional" | "enterprise"): FeaturesResponse {
  const base = {
    plan,
    max_requests_per_month: plan === "professional" ? 100_000 : Number.MAX_SAFE_INTEGER,
    logs_enabled: plan !== "community",
    traces_enabled: plan !== "community",
    feedback_enabled: plan !== "community",
    alerts_enabled: plan !== "community",
    finops_dashboard_enabled: plan === "enterprise",
    retention_days: plan === "professional" ? 30 : plan === "enterprise" ? 2_000_000 : 0,
    llm_key_management: plan !== "community",
    simple_cache_enabled: plan !== "community",
    semantic_cache_enabled: plan !== "community",
    prompt_templates_enabled: plan !== "community",
    max_prompt_templates: plan !== "community" ? Number.MAX_SAFE_INTEGER : 0,
    playground_enabled: plan !== "community",
    prompt_versioning_enabled: plan !== "community",
    deterministic_guardrails: plan !== "community",
    partner_guardrails: plan !== "community",
    pii_redaction_enabled: plan !== "community",
    rbac_enabled: plan !== "community",
    team_management: plan !== "community",
    audit_logs_enabled: plan === "enterprise",
    scim_provisioning: plan === "enterprise",
    jwt_auth_enabled: plan === "enterprise",
    byok_enabled: plan === "enterprise",
    sso_enabled: plan === "enterprise",
    org_management_enabled: plan === "enterprise",
    datalake_export_enabled: plan === "enterprise",
    private_llm_cloud: plan === "enterprise",
    autonomous_fine_tuning: plan === "enterprise",
  } as unknown as FeaturesResponse["features"]
  return { plan, features: base }
}

describe("<FeatureGate>", () => {
  beforeEach(() => vi.clearAllMocks())

  it("renders children when the feature is enabled", async () => {
    mockApi.getFeatures.mockResolvedValue(featuresFor("enterprise"))

    renderGated(
      <FeatureGate feature="sso_enabled" title="SSO" requiredPlan="enterprise">
        <div>SSO admin page</div>
      </FeatureGate>
    )

    await waitFor(() => {
      expect(screen.getByText("SSO admin page")).toBeInTheDocument()
    })
  })

  it("renders upsell when the feature is gated off", async () => {
    mockApi.getFeatures.mockResolvedValue(featuresFor("community"))

    renderGated(
      <FeatureGate feature="feedback_enabled" title="Feedback" requiredPlan="professional">
        <div>Feedback page content</div>
      </FeatureGate>
    )

    await waitFor(() => {
      expect(screen.queryByText("Feedback page content")).not.toBeInTheDocument()
      expect(screen.getByText(/isn't available on the/i)).toBeInTheDocument()
      expect(screen.getByText(/Open Source/)).toBeInTheDocument()
      // Required plan badge
      expect(screen.getByText(/Requires Professional/i)).toBeInTheDocument()
    })
  })

  it("community user cannot access enterprise features", async () => {
    mockApi.getFeatures.mockResolvedValue(featuresFor("community"))

    renderGated(
      <FeatureGate feature="sso_enabled" title="SSO" requiredPlan="enterprise">
        <div>SSO page</div>
      </FeatureGate>
    )

    await waitFor(() => {
      expect(screen.queryByText("SSO page")).not.toBeInTheDocument()
      expect(screen.getByText(/Requires Enterprise/i)).toBeInTheDocument()
    })
  })

  it("professional user cannot access enterprise features", async () => {
    mockApi.getFeatures.mockResolvedValue(featuresFor("professional"))

    renderGated(
      <FeatureGate feature="org_management_enabled" title="Organizations" requiredPlan="enterprise">
        <div>Org page</div>
      </FeatureGate>
    )

    await waitFor(() => {
      expect(screen.queryByText("Org page")).not.toBeInTheDocument()
      expect(screen.getByText(/Professional/)).toBeInTheDocument()
      expect(screen.getByText(/Requires Enterprise/i)).toBeInTheDocument()
    })
  })

  it("professional user can access pro features", async () => {
    mockApi.getFeatures.mockResolvedValue(featuresFor("professional"))

    renderGated(
      <FeatureGate feature="feedback_enabled" title="Feedback" requiredPlan="professional">
        <div>Feedback content</div>
      </FeatureGate>
    )

    await waitFor(() => {
      expect(screen.getByText("Feedback content")).toBeInTheDocument()
    })
  })
})
