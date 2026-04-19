import { describe, it, expect, beforeEach } from "vitest"
import {
  getToken, setTokens, clearAuth, getTenantId, setTenantId,
  getRefreshToken, sanitizeApiError, ssoAuthorizeUrl,
} from "./api"
import { AxiosError } from "axios"

describe("api token helpers", () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it("round-trips access + refresh tokens", () => {
    setTokens("access-abc", "refresh-xyz")
    expect(getToken()).toBe("access-abc")
    expect(getRefreshToken()).toBe("refresh-xyz")
  })

  it("round-trips tenant id", () => {
    setTenantId("tenant-uuid")
    expect(getTenantId()).toBe("tenant-uuid")
  })

  it("clearAuth removes access, refresh, tenant", () => {
    setTokens("a", "b")
    setTenantId("t")
    clearAuth()
    expect(getToken()).toBeNull()
    expect(getRefreshToken()).toBeNull()
    expect(getTenantId()).toBeNull()
  })

  it("returns null when no tokens set", () => {
    expect(getToken()).toBeNull()
    expect(getRefreshToken()).toBeNull()
    expect(getTenantId()).toBeNull()
  })
})

describe("sanitizeApiError", () => {
  it("prefers backend-provided error message", () => {
    const err = Object.assign(new AxiosError("http"), {
      response: { status: 400, data: { error: "Invalid email format" } },
    } as unknown as Record<string, unknown>) as AxiosError
    expect(sanitizeApiError(err)).toBe("Invalid email format")
  })

  it.each([
    [401, "Authentication required. Please log in."],
    [403, "You don't have permission for this action."],
    [404, "The requested resource was not found."],
    [409, "A conflict occurred. The resource may already exist."],
    [422, "Validation failed. Please check your input."],
    [423, "Account temporarily locked. Try again later."],
    [429, "Too many requests. Please try again later."],
    [500, "A server error occurred. Please try again."],
    [502, "A server error occurred. Please try again."],
  ])("maps status %i to %s", (status, expected) => {
    const err = Object.assign(new AxiosError("http"), {
      response: { status, data: {} },
    } as unknown as Record<string, unknown>) as AxiosError
    expect(sanitizeApiError(err)).toBe(expected)
  })

  it("falls back for non-Axios errors", () => {
    expect(sanitizeApiError(new Error("boom"))).toBe("boom")
    expect(sanitizeApiError("string error")).toBe("An unexpected error occurred.")
  })
})

describe("ssoAuthorizeUrl", () => {
  it("builds a well-formed authorize URL with tenant", () => {
    const u = ssoAuthorizeUrl("keycloak-main", "acme")
    expect(u).toContain("/auth/sso/keycloak-main/authorize")
    expect(u).toContain("tenant=acme")
  })

  it("includes redirect_after when provided", () => {
    const u = ssoAuthorizeUrl("okta", "acme", "/dashboard")
    expect(u).toContain("redirect_after=%2Fdashboard")
  })

  it("url-encodes slug with special chars", () => {
    const u = ssoAuthorizeUrl("a b", "t")
    expect(u).toContain("/auth/sso/a%20b/authorize")
  })
})
