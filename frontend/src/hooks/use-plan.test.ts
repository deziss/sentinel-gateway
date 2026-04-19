import { describe, it, expect } from "vitest"
import { planMeets } from "./use-plan"

describe("planMeets", () => {
  it("enterprise meets all tiers", () => {
    expect(planMeets("enterprise", "community")).toBe(true)
    expect(planMeets("enterprise", "professional")).toBe(true)
    expect(planMeets("enterprise", "enterprise")).toBe(true)
  })

  it("professional meets community and professional only", () => {
    expect(planMeets("professional", "community")).toBe(true)
    expect(planMeets("professional", "professional")).toBe(true)
    expect(planMeets("professional", "enterprise")).toBe(false)
  })

  it("community only meets community", () => {
    expect(planMeets("community", "community")).toBe(true)
    expect(planMeets("community", "professional")).toBe(false)
    expect(planMeets("community", "enterprise")).toBe(false)
  })

  it("undefined plan never meets any tier", () => {
    expect(planMeets(undefined, "community")).toBe(false)
    expect(planMeets(undefined, "professional")).toBe(false)
    expect(planMeets(undefined, "enterprise")).toBe(false)
  })
})
