import { describe, it, expect } from "vitest"
import * as E from "fp-ts/Either"
import { required, minLength, email, url, validate } from "./fp-validate"

describe("fp-validate", () => {
  describe("required", () => {
    it("accepts non-empty strings", () => {
      expect(E.isRight(required("Name")("Alice"))).toBe(true)
    })

    it("rejects empty strings", () => {
      const r = required("Name")("")
      expect(E.isLeft(r)).toBe(true)
      if (E.isLeft(r)) expect(r.left[0].field).toBe("Name")
    })

    it("rejects whitespace-only strings", () => {
      expect(E.isLeft(required("Name")("   "))).toBe(true)
    })
  })

  describe("minLength", () => {
    it("accepts strings meeting the minimum", () => {
      expect(E.isRight(minLength("Password", 8)("abcdefgh"))).toBe(true)
    })

    it("rejects strings below the minimum", () => {
      const r = minLength("Password", 8)("short")
      expect(E.isLeft(r)).toBe(true)
      if (E.isLeft(r)) expect(r.left[0].message).toContain("at least 8")
    })
  })

  describe("email", () => {
    it.each([
      ["alice@example.com", true],
      ["user+tag@domain.co", true],
      ["not-an-email", false],
      ["", false],
      ["@example.com", false],
      ["alice@", false],
    ])("%s → valid=%s", (input, valid) => {
      const r = email("Email")(input)
      expect(E.isRight(r)).toBe(valid)
    })
  })

  describe("url", () => {
    it.each([
      ["https://example.com", true],
      ["http://localhost:3000/path", true],
      ["not a url", false],
      ["", false],
    ])("%s → valid=%s", (input, valid) => {
      expect(E.isRight(url("URL")(input))).toBe(valid)
    })
  })

  describe("validate", () => {
    it("passes when all validators succeed", () => {
      const r = validate("alice@example.com", required("Email"), email("Email"))
      expect(E.isRight(r)).toBe(true)
    })

    it("aggregates errors from multiple validators", () => {
      const r = validate("", required("Email"), email("Email"))
      expect(E.isLeft(r)).toBe(true)
      if (E.isLeft(r)) expect(r.left.length).toBe(2)
    })

    it("returns only failing validator errors", () => {
      const r = validate("not-email", required("Email"), email("Email"))
      expect(E.isLeft(r)).toBe(true)
      if (E.isLeft(r)) expect(r.left.length).toBe(1)
    })
  })
})
