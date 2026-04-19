import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor } from "@testing-library/react"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { Organizations } from "./Organizations"
import * as api from "@/lib/api"

vi.mock("@/lib/api", async () => {
  const mod = await vi.importActual<typeof import("@/lib/api")>("@/lib/api")
  return { ...mod, listOrganizations: vi.fn(), getFeatures: vi.fn() }
})

const mockApi = api as unknown as {
  listOrganizations: ReturnType<typeof vi.fn>
  getFeatures: ReturnType<typeof vi.fn>
}

/** Enterprise plan = full features. Used so the FeatureGate lets content through. */
function enterpriseFeatures() {
  return {
    plan: "enterprise" as const,
    features: {
      plan: "enterprise",
      org_management_enabled: true,
      feedback_enabled: true,
    } as unknown as import("@/lib/api").FeatureFlags,
  }
}

function renderWithQuery(ui: React.ReactElement) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>)
}

describe("Organizations page", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockApi.getFeatures.mockResolvedValue(enterpriseFeatures())
  })

  it("renders heading and new button", async () => {
    mockApi.listOrganizations.mockResolvedValue([])
    renderWithQuery(<Organizations />)
    await waitFor(() => {
      expect(screen.getByText("Organizations")).toBeInTheDocument()
      expect(screen.getByRole("button", { name: /New Organization/i })).toBeInTheDocument()
    })
  })

  it("shows empty state when no orgs exist", async () => {
    mockApi.listOrganizations.mockResolvedValue([])
    renderWithQuery(<Organizations />)
    await waitFor(() => {
      expect(screen.getByText("No organizations yet")).toBeInTheDocument()
    })
  })

  it("renders organizations in the table", async () => {
    mockApi.listOrganizations.mockResolvedValue([
      {
        id: "org-1",
        slug: "acme",
        name: "Acme Corp",
        plan: "pro",
        metadata: {},
        is_active: true,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ])
    renderWithQuery(<Organizations />)
    await waitFor(() => {
      expect(screen.getByText("Acme Corp")).toBeInTheDocument()
      expect(screen.getByText("acme")).toBeInTheDocument()
      expect(screen.getByText("pro")).toBeInTheDocument()
    })
  })
})
