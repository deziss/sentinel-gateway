import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor } from "@testing-library/react"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { Feedback } from "./Feedback"
import * as api from "@/lib/api"

vi.mock("@/lib/api", async () => {
  const mod = await vi.importActual<typeof import("@/lib/api")>("@/lib/api")
  return {
    ...mod,
    listFeedback: vi.fn(),
    getFeedbackStats: vi.fn(),
    getFeatures: vi.fn(),
  }
})

const mockApi = api as unknown as {
  listFeedback: ReturnType<typeof vi.fn>
  getFeedbackStats: ReturnType<typeof vi.fn>
  getFeatures: ReturnType<typeof vi.fn>
}

/** Professional plan — feedback enabled. */
function proFeatures() {
  return {
    plan: "professional" as const,
    features: {
      plan: "professional",
      feedback_enabled: true,
    } as unknown as import("@/lib/api").FeatureFlags,
  }
}

function renderWithQuery(ui: React.ReactElement) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>)
}

describe("Feedback page", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockApi.getFeatures.mockResolvedValue(proFeatures())
  })

  it("renders stats cards with fetched data", async () => {
    mockApi.getFeedbackStats.mockResolvedValue({
      total: 42,
      positive: 30,
      negative: 12,
      positive_ratio: 30 / 42,
      window_days: 30,
    })
    mockApi.listFeedback.mockResolvedValue([])

    renderWithQuery(<Feedback />)

    await waitFor(() => {
      expect(screen.getByText("LLM Feedback")).toBeInTheDocument()
      expect(screen.getByText("42")).toBeInTheDocument()
      expect(screen.getByText("30")).toBeInTheDocument()
      expect(screen.getByText("12")).toBeInTheDocument()
      expect(screen.getByText("71%")).toBeInTheDocument()
    })
  })

  it("shows empty state when no feedback exists", async () => {
    mockApi.getFeedbackStats.mockResolvedValue({
      total: 0, positive: 0, negative: 0, positive_ratio: 0, window_days: 30,
    })
    mockApi.listFeedback.mockResolvedValue([])

    renderWithQuery(<Feedback />)

    await waitFor(() => {
      expect(screen.getByText(/No feedback yet/i)).toBeInTheDocument()
    })
  })

  it("renders feedback rows with rating badges", async () => {
    mockApi.getFeedbackStats.mockResolvedValue({
      total: 2, positive: 1, negative: 1, positive_ratio: 0.5, window_days: 30,
    })
    mockApi.listFeedback.mockResolvedValue([
      {
        id: "fb-1",
        tenant_id: "t",
        user_id: null,
        llm_log_id: "log-1",
        request_id: null,
        rating: 1,
        comment: "Great response",
        metadata: {},
        created_at: new Date().toISOString(),
      },
      {
        id: "fb-2",
        tenant_id: "t",
        user_id: null,
        llm_log_id: null,
        request_id: "req-123",
        rating: -1,
        comment: null,
        metadata: {},
        created_at: new Date().toISOString(),
      },
    ])

    renderWithQuery(<Feedback />)

    await waitFor(() => {
      expect(screen.getByText("Great response")).toBeInTheDocument()
      expect(screen.getByText("req-123")).toBeInTheDocument()
      expect(screen.getByText("+1")).toBeInTheDocument()
      expect(screen.getByText("-1")).toBeInTheDocument()
    })
  })

  it("shows error state when API fails", async () => {
    mockApi.getFeedbackStats.mockRejectedValue(new Error("Network error"))
    mockApi.listFeedback.mockRejectedValue(new Error("Network error"))

    renderWithQuery(<Feedback />)

    await waitFor(() => {
      expect(screen.getByText(/Failed to load feedback/i)).toBeInTheDocument()
    })
  })
})
