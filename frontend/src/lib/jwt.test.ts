import { describe, it, expect } from "vitest"
import * as E from "fp-ts/Either"
import * as O from "fp-ts/Option"
import { decodeJwt, getClaimsFromToken } from "./jwt"

/** Build a fake JWT string. We only care about the payload — signature is ignored. */
function makeJwt(payload: Record<string, unknown>): string {
  const header = btoa(JSON.stringify({ alg: "RS256", typ: "JWT" }))
  const body = btoa(JSON.stringify(payload))
  return `${header}.${body}.signature`
}

describe("decodeJwt", () => {
  it("extracts claims from a well-formed token", () => {
    const token = makeJwt({
      sub: "user-1",
      tid: "tenant-1",
      role: "tenant_admin",
      typ: "access",
      exp: 1_700_000_000,
      iat: 1_699_000_000,
      jti: "jti-1",
    })
    const r = decodeJwt(token)
    expect(E.isRight(r)).toBe(true)
    if (E.isRight(r)) {
      expect(r.right.sub).toBe("user-1")
      expect(r.right.role).toBe("tenant_admin")
    }
  })

  it("rejects malformed tokens", () => {
    expect(E.isLeft(decodeJwt("not.a.valid.jwt.structure"))).toBe(true)
    expect(E.isLeft(decodeJwt("only-two.parts"))).toBe(true)
    expect(E.isLeft(decodeJwt("garbage"))).toBe(true)
  })

  it("rejects tokens with invalid base64 in payload", () => {
    expect(E.isLeft(decodeJwt("header.!!!.sig"))).toBe(true)
  })
})

describe("getClaimsFromToken", () => {
  it("returns None for null token", () => {
    expect(O.isNone(getClaimsFromToken(null))).toBe(true)
  })

  it("returns None for invalid token", () => {
    expect(O.isNone(getClaimsFromToken("garbage"))).toBe(true)
  })

  it("returns Some for valid token", () => {
    const token = makeJwt({
      sub: "u", tid: "t", role: "user", typ: "access",
      exp: 0, iat: 0, jti: "j",
    })
    expect(O.isSome(getClaimsFromToken(token))).toBe(true)
  })
})
